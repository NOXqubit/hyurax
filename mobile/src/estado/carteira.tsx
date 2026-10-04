// O estado da carteira no aparelho.
//
// Onde fica cada coisa:
// - o ARQUIVO da carteira (formato HYURAX-CARTEIRA-v2, cifrado com a senha)
//   e o SEGREDO ficam no SecureStore, que no Android é cifrado pelo
//   Keystore do aparelho. O segredo é guardado sem exigir biometria no
//   armazenamento, de propósito: trocar a digital invalidaria a chave e o
//   saldo ficaria preso. Quem protege o uso é a confirmação do aparelho
//   (digital, rosto ou PIN) pedida antes de cada envio;
// - o arquivo cifrado é a cópia de segurança: abre no programa do PC e em
//   outro celular, com a senha.

import * as Crypto from "expo-crypto";
import * as LocalAuthentication from "expo-local-authentication";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { deHex, hex } from "../nucleo/bytes";
import * as armazem from "./armazem";
import * as arquivo from "../nucleo/carteira";
import * as endereco from "../nucleo/endereco";
import * as no from "../nucleo/no";
import * as tx from "../nucleo/transacao";

const CHAVE_ARQUIVO = "hyurax.carteira.arquivo";
const CHAVE_SEGREDO = "hyurax.carteira.segredo";
const CHAVE_NO = "hyurax.no.endereco";
const REDE_PADRAO = "hyurax-testnet";

type Situacao = "carregando" | "sem-carteira" | "pronta";

type Estado = {
  situacao: Situacao;
  /** Os 20 bytes do endereço, em hexadecimal. */
  enderecoHex: string | null;
  /** O endereço como as pessoas veem, na rede do nó (ou de teste, sem nó). */
  enderecoTexto: string | null;
  rede: string;
  noEndereco: string | null;
  info: no.Info | null;
  conta: no.Conta | null;
  movimentos: no.Movimento[];
  atualizando: boolean;
  erro: string | null;
  atualizado: number | null;
};

type Acoes = {
  criar(senha: string, aoAvancar?: (f: number) => void): Promise<void>;
  importar(texto: string, senha: string, aoAvancar?: (f: number) => void): Promise<void>;
  apagar(): Promise<void>;
  arquivoDeBackup(): Promise<string | null>;
  definirNo(entrada: string): Promise<no.Info>;
  atualizar(): Promise<void>;
  enviar(destino: string, valor: bigint, taxa: bigint): Promise<string>;
  confirmarDono(motivo: string): Promise<boolean>;
};

const Contexto = createContext<(Estado & Acoes) | null>(null);

export function useCarteira(): Estado & Acoes {
  const c = useContext(Contexto);
  if (!c) throw new Error("useCarteira fora do ProvedorDaCarteira");
  return c;
}

const ESTADO_INICIAL: Estado = {
  situacao: "carregando",
  enderecoHex: null,
  enderecoTexto: null,
  rede: REDE_PADRAO,
  noEndereco: null,
  info: null,
  conta: null,
  movimentos: [],
  atualizando: false,
  erro: null,
  atualizado: null,
};

async function guardar(texto: string, segredo: Uint8Array): Promise<void> {
  await armazem.gravar(CHAVE_ARQUIVO, texto);
  await armazem.gravar(CHAVE_SEGREDO, hex(segredo));
}

export function ProvedorDaCarteira({ children }: { children: ReactNode }) {
  const [e, setE] = useState<Estado>(ESTADO_INICIAL);
  const atual = useRef(e);
  atual.current = e;
  const mudar = useCallback((p: Partial<Estado>) => setE((x) => ({ ...x, ...p })), []);

  const textoDoEndereco = (h: string | null, rede: string) => {
    const b = h ? deHex(h) : null;
    return b ? endereco.mostrar(b, rede) : null;
  };

  // abertura: o que já está guardado no aparelho
  useEffect(() => {
    (async () => {
      const [texto, noSalvo] = await Promise.all([armazem.ler(CHAVE_ARQUIVO), armazem.ler(CHAVE_NO)]);
      if (!texto) {
        mudar({ situacao: "sem-carteira", noEndereco: noSalvo });
        return;
      }
      const h = hex(arquivo.enderecoDoArquivo(texto));
      mudar({ situacao: "pronta", enderecoHex: h, enderecoTexto: textoDoEndereco(h, REDE_PADRAO), noEndereco: noSalvo });
    })().catch((err) => mudar({ situacao: "sem-carteira", erro: String(err?.message ?? err) }));
  }, [mudar]);

  const atualizar = useCallback(async () => {
    const { noEndereco, enderecoHex } = atual.current;
    if (!noEndereco || !enderecoHex) return;
    mudar({ atualizando: true });
    try {
      const info = await no.info(noEndereco);
      const texto = textoDoEndereco(enderecoHex, info.rede)!;
      const [conta, movimentos] = await Promise.all([no.conta(noEndereco, texto), no.historico(noEndereco, texto)]);
      mudar({ info, rede: info.rede, enderecoTexto: texto, conta, movimentos, erro: null, atualizado: Date.now() });
    } catch (err: any) {
      mudar({ erro: String(err?.message ?? err) });
    } finally {
      mudar({ atualizando: false });
    }
  }, [mudar]);

  // de 15 em 15 segundos, enquanto houver carteira e nó
  useEffect(() => {
    if (e.situacao !== "pronta" || !e.noEndereco) return;
    atualizar();
    const t = setInterval(atualizar, 15_000);
    return () => clearInterval(t);
  }, [e.situacao, e.noEndereco, atualizar]);

  const acoes: Acoes = useMemo(
    () => ({
      async criar(senha, aoAvancar) {
        arquivo.senhaAceitavel(senha);
        const segredo = Crypto.getRandomBytes(32);
        const texto = await arquivo.cifrar(segredo, senha, Crypto.getRandomBytes(28), aoAvancar);
        await guardar(texto, segredo);
        const h = hex(tx.enderecoDoSegredo(segredo));
        mudar({ situacao: "pronta", enderecoHex: h, enderecoTexto: textoDoEndereco(h, atual.current.rede), conta: null, movimentos: [] });
      },
      async importar(texto, senha, aoAvancar) {
        const limpo = texto.replace(/\r\n/g, "\n").trim() + "\n";
        const segredo = await arquivo.abrir(limpo, senha, aoAvancar);
        // arquivo antigo (sem senha): já sai cifrado com a senha dada
        const final = limpo.includes("formato=") ? limpo : await arquivo.cifrar(segredo, senha, Crypto.getRandomBytes(28), aoAvancar);
        await guardar(final, segredo);
        const h = hex(tx.enderecoDoSegredo(segredo));
        mudar({ situacao: "pronta", enderecoHex: h, enderecoTexto: textoDoEndereco(h, atual.current.rede), conta: null, movimentos: [] });
      },
      async apagar() {
        await armazem.apagar(CHAVE_SEGREDO);
        await armazem.apagar(CHAVE_ARQUIVO);
        mudar({ ...ESTADO_INICIAL, situacao: "sem-carteira", noEndereco: atual.current.noEndereco });
      },
      arquivoDeBackup: () => armazem.ler(CHAVE_ARQUIVO),
      async definirNo(entrada) {
        const base = no.normalizarEndereco(entrada);
        const info = await no.info(base);
        await armazem.gravar(CHAVE_NO, base);
        mudar({ noEndereco: base, info, rede: info.rede, enderecoTexto: textoDoEndereco(atual.current.enderecoHex, info.rede) });
        return info;
      },
      atualizar,
      async confirmarDono(motivo) {
        const tem = (await LocalAuthentication.hasHardwareAsync()) && (await LocalAuthentication.isEnrolledAsync());
        if (!tem) return true; // aparelho sem trava: nada a pedir (a tela avisa)
        const r = await LocalAuthentication.authenticateAsync({ promptMessage: motivo, cancelLabel: "Cancelar" });
        return r.success;
      },
      async enviar(destino, valor, taxa) {
        const { noEndereco, info } = atual.current;
        if (!noEndereco || !info) throw new Error("conecte um nó em Ajustes antes de enviar");
        const para = endereco.ler(destino, info.rede);
        const segredoHex = await armazem.ler(CHAVE_SEGREDO);
        const segredo = segredoHex ? deHex(segredoHex) : null;
        if (!segredo || segredo.length !== 32) throw new Error("o segredo desta carteira não está no aparelho; importe de novo");
        // nonce e saldo de agora, não os da última atualização
        const conta = await no.conta(noEndereco, endereco.mostrar(tx.enderecoDoSegredo(segredo), info.rede));
        if (valor + taxa > conta.saldo) throw new Error("saldo gastável insuficiente");
        const t = tx.assinar(segredo, info.magic, [{ destino: para, ativo: tx.HYX, valor }], taxa, conta.proximoNonce);
        segredo.fill(0);
        const id = await no.enviar(noEndereco, t);
        atualizar();
        return id;
      },
    }),
    [mudar, atualizar],
  );

  return <Contexto.Provider value={{ ...e, ...acoes }}>{children}</Contexto.Provider>;
}
