# ✝ Isaías 26:20 — “Vai, pois, povo meu, entra nos teus quartos e fecha as tuas portas sobre ti; esconde-te só por um momento, até que passe a ira.”
"""AURON — estado de contas: saldos por ativo, nonces, maturação de coinbase.

Corrige três coisas do protótipo.

BUG 9 (parte) — emissão infinita.
  A coinbase podia pagar até `MAX_COINBASE_AMOUNT`, que era a recompensa fixa
  de 50 AUR, para sempre. Aqui a recompensa vem de `block_reward(height)`,
  com halving, e o total emitido é conferido contra o teto de 21 milhões a
  cada bloco.

BUG novo que o protótipo tinha sem perceber — taxas não existiam.
  Sem taxa, o mempool não tem como priorizar nem se defender de flood, e o
  minerador não tem receita quando a recompensa zerar.

Maturação de coinbase.
  O protótipo creditava a recompensa na hora. Se a cadeia reorganiza, quem
  aceitou um pagamento financiado por aquela recompensa fica sem lastro.
  Aqui a recompensa fica retida por `coinbase_maturity` blocos antes de
  virar saldo gastável.

Vários ativos.
  O saldo é guardado por conta e por ativo: a chave é o par (endereço,
  identificador do ativo). O nonce continua por conta. Hoje só o AUR existe,
  e a invariante recusa qualquer outro ativo que apareça no estado.

`apply_block` devolve um registro de desfazer. Sem ele não existe reorg
correto, e sem reorg correto não existe rede P2P.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from .consensus import ChainParams, ConsensusError, block_reward
from .tx import AUR, KNOWN_ASSETS, Coinbase, Transfer, TxError
from .units import MAX_SUPPLY, checked_add, checked_sub


class StateError(Exception):
    """Regra de estado violada."""


@dataclass
class Undo:
    """Tudo que é preciso para desfazer exatamente um bloco."""

    height: int
    balances: dict = field(default_factory=dict)   # (addr, ativo) -> valor anterior ou None
    nonces: dict = field(default_factory=dict)     # addr -> valor anterior ou None
    matured_at: int | None = None                  # altura que amadureceu
    matured_entries: list = field(default_factory=list)
    pending_added: int | None = None               # altura adicionada ao pending
    total_emitted_before: int = 0


@dataclass
class State:
    params: ChainParams
    # (endereço, identificador do ativo) -> saldo
    balances: dict = field(default_factory=dict)
    nonces: dict = field(default_factory=dict)
    # altura -> lista de (endereço, valor em AUR) esperando maturar
    pending_coinbase: dict = field(default_factory=dict)
    total_emitted: int = 0

    # -- leitura --

    def balance(self, address: bytes, asset: bytes = AUR) -> int:
        return self.balances.get((address, asset), 0)

    def next_nonce(self, address: bytes) -> int:
        return self.nonces.get(address, 0)

    def immature_balance(self, address: bytes) -> int:
        total = 0
        for entries in self.pending_coinbase.values():
            for addr, amount in entries:
                if addr == address:
                    total += amount
        return total

    def total_balance(self, asset: bytes = AUR) -> int:
        return sum(v for (_, a), v in self.balances.items() if a == asset)

    # -- escrita interna, com registro de desfazer --

    def _set_balance(self, undo: Undo, address: bytes, asset: bytes, value: int) -> None:
        key = (address, asset)
        if key not in undo.balances:
            undo.balances[key] = self.balances.get(key)
        if value:
            self.balances[key] = value
        else:
            self.balances.pop(key, None)

    def _set_nonce(self, undo: Undo, address: bytes, value: int) -> None:
        if address not in undo.nonces:
            undo.nonces[address] = self.nonces.get(address)
        self.nonces[address] = value

    # -- aplicação --

    def apply_block(self, height: int, transactions: list, network_magic: bytes) -> Undo:
        """Aplica um bloco inteiro. Ou tudo entra, ou nada entra.

        A ordem importa: primeiro a coinbase antiga amadurece, depois as
        transferências do bloco rodam. Assim a recompensa da altura H fica
        gastável exatamente a partir da altura H + maturity.
        """
        if not transactions:
            raise StateError("bloco sem transações; falta a coinbase")

        coinbase, *transfers = transactions
        if not isinstance(coinbase, Coinbase):
            raise StateError("a primeira transação do bloco precisa ser a coinbase")
        for tx in transfers:
            if isinstance(tx, Coinbase):
                raise StateError("coinbase extra rejeitada")

        undo = Undo(height=height, total_emitted_before=self.total_emitted)

        # Fotografia completa antes de mexer em qualquer coisa. Se qualquer
        # transação do bloco falhar, o estado volta inteiro. Um bloco meio
        # aplicado é pior do que um bloco rejeitado.
        snap_balances = dict(self.balances)
        snap_nonces = dict(self.nonces)
        snap_pending = {h: list(v) for h, v in self.pending_coinbase.items()}
        snap_emitted = self.total_emitted

        try:
            self._mature(undo, height)
            fees = self._apply_transfers(undo, transfers, network_magic)
            self._apply_coinbase(undo, coinbase, height, fees)
        except Exception:
            self.balances = snap_balances
            self.nonces = snap_nonces
            self.pending_coinbase = snap_pending
            self.total_emitted = snap_emitted
            raise

        return undo

    def _mature(self, undo: Undo, height: int) -> None:
        target = height - self.params.coinbase_maturity
        entries = self.pending_coinbase.pop(target, None)
        if entries is None:
            return
        undo.matured_at = target
        undo.matured_entries = list(entries)
        for address, amount in entries:
            self._set_balance(undo, address, AUR, checked_add(self.balance(address), amount))

    def _apply_transfers(self, undo: Undo, transfers: list,
                         network_magic: bytes) -> int:
        total_fees = 0
        seen: set[bytes] = set()

        for tx in transfers:
            if not isinstance(tx, Transfer):
                raise StateError("tipo de transação inesperado no bloco")

            txid = tx.txid()
            if txid in seen:
                raise StateError("transação duplicada dentro do mesmo bloco")
            seen.add(txid)

            ok, msg = tx.check_signature(network_magic)
            if not ok:
                raise StateError(f"transação inválida: {msg}")

            expected = self.next_nonce(tx.sender)
            if tx.nonce != expected:
                raise StateError(
                    f"nonce inválido: esperado {expected}, veio {tx.nonce} (replay?)"
                )

            # Primeiro confere todos os ativos, depois mexe em qualquer um:
            # uma transferência sem saldo num ativo não pode debitar os outros.
            costs = tx.costs()
            for asset, cost in sorted(costs.items()):
                if self.balance(tx.sender, asset) < cost:
                    raise StateError("saldo insuficiente")
            for asset, cost in sorted(costs.items()):
                self._set_balance(
                    undo, tx.sender, asset, checked_sub(self.balance(tx.sender, asset), cost)
                )
            for output in tx.outputs:
                self._set_balance(
                    undo, output.recipient, output.asset_id,
                    checked_add(self.balance(output.recipient, output.asset_id), output.amount),
                )
            self._set_nonce(undo, tx.sender, expected + 1)
            total_fees = checked_add(total_fees, tx.fee)

        return total_fees

    def _apply_coinbase(self, undo: Undo, coinbase: Coinbase,
                        height: int, fees: int) -> None:
        if coinbase.height != height:
            raise StateError(
                f"coinbase declara altura {coinbase.height}, bloco está em {height}"
            )
        if coinbase.amount <= 0:
            raise StateError("coinbase sem valor")

        subsidy = block_reward(height, self.params)
        allowed = checked_add(subsidy, fees)
        if coinbase.amount > allowed:
            raise StateError(
                f"coinbase pede {coinbase.amount}, máximo é {allowed} "
                f"(subsídio {subsidy} + taxas {fees})"
            )

        # O subsídio é emissão nova; a parte de taxa é dinheiro que já existia.
        minted = min(coinbase.amount, subsidy)
        if checked_add(self.total_emitted, minted) > MAX_SUPPLY:
            raise StateError("emissão passaria do teto de supply")
        self.total_emitted += minted

        undo.pending_added = height
        self.pending_coinbase.setdefault(height, []).append(
            (coinbase.recipient, coinbase.amount)
        )

    # -- desfazer --

    def revert_block(self, undo: Undo) -> None:
        """Desfaz exatamente um bloco, na ordem inversa."""
        if undo.pending_added is not None:
            entries = self.pending_coinbase.get(undo.pending_added)
            if entries:
                entries.pop()
                if not entries:
                    self.pending_coinbase.pop(undo.pending_added, None)

        for key, previous in undo.balances.items():
            if previous is None:
                self.balances.pop(key, None)
            else:
                self.balances[key] = previous

        for address, previous in undo.nonces.items():
            if previous is None:
                self.nonces.pop(address, None)
            else:
                self.nonces[address] = previous

        if undo.matured_at is not None:
            self.pending_coinbase[undo.matured_at] = list(undo.matured_entries)

        self.total_emitted = undo.total_emitted_before

    # -- utilidades --

    def copy(self) -> "State":
        return State(
            params=self.params,
            balances=dict(self.balances),
            nonces=dict(self.nonces),
            pending_coinbase={h: list(v) for h, v in self.pending_coinbase.items()},
            total_emitted=self.total_emitted,
        )

    def check_invariants(self) -> None:
        """Invariantes que precisam valer depois de qualquer bloco."""
        for (address, asset), value in self.balances.items():
            if value < 0:
                raise StateError(f"saldo negativo em {address.hex()}")
            if asset not in KNOWN_ASSETS:
                raise StateError(f"saldo em ativo sem regra de emissão: {asset.hex()}")
        if self.total_emitted > MAX_SUPPLY:
            raise StateError("emissão total passou do teto")
        circulating = self.total_balance(AUR) + sum(
            amount for entries in self.pending_coinbase.values() for _, amount in entries
        )
        if circulating > MAX_SUPPLY:
            raise StateError(
                f"moeda em circulação ({circulating}) passou do teto ({MAX_SUPPLY})"
            )
