// Enviar: destino (colado ou do QR), valor, revisão, confirmação do dono
// do aparelho, e só então a assinatura aqui e a entrega ao nó.

import * as Clipboard from "expo-clipboard";
import { router, useLocalSearchParams } from "expo-router";
import { useEffect, useMemo, useState } from "react";
import { View } from "react-native";

import { useCarteira } from "../estado/carteira";
import * as endereco from "../nucleo/endereco";
import { textoDeUnidades, unidadesDeTexto } from "../nucleo/valor";
import { Aviso, Botao, Campo, Cartao, Linha, SeloTeste, Tela, Texto, Titulo } from "../ui/componentes";

export default function Enviar() {
  const c = useCarteira();
  const lido = useLocalSearchParams<{ destino?: string }>();
  const [destino, setDestino] = useState("");
  const [valor, setValor] = useState("");
  const [revisando, setRevisando] = useState(false);
  const [enviando, setEnviando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [feito, setFeito] = useState<string | null>(null);

  useEffect(() => {
    if (lido.destino) setDestino(String(lido.destino));
  }, [lido.destino]);

  const rede = c.info?.rede ?? c.rede;
  const taxa = c.info?.taxaSugerida ?? 0n;
  const conferido = useMemo(() => {
    let erroDestino: string | null = null;
    let erroValor: string | null = null;
    let unidades = 0n;
    if (destino.trim()) {
      try {
        const b = endereco.ler(destino, rede);
        if (c.enderecoHex && [...b].map((x) => x.toString(16).padStart(2, "0")).join("") === c.enderecoHex) erroDestino = "esse é o seu próprio endereço";
      } catch (e: any) {
        erroDestino = e.message;
      }
    }
    if (valor.trim()) {
      try {
        unidades = unidadesDeTexto(valor);
        if (unidades === 0n) erroValor = "o valor precisa ser maior que zero";
        else if (c.conta && unidades + taxa > c.conta.saldo) erroValor = `maior que o saldo gastável (${textoDeUnidades(c.conta.saldo)} HYX)`;
      } catch (e: any) {
        erroValor = e.message;
      }
    }
    return { erroDestino, erroValor, unidades, pronto: !!destino.trim() && !!valor.trim() && !erroDestino && !erroValor };
  }, [destino, valor, rede, c.conta, c.enderecoHex, taxa]);

  async function confirmar() {
    setErro(null);
    if (!(await c.confirmarDono("Confirme para enviar HYX"))) return setErro("Envio cancelado.");
    setEnviando(true);
    try {
      setFeito(await c.enviar(destino, conferido.unidades, taxa));
    } catch (e: any) {
      setErro(String(e?.message ?? e));
    } finally {
      setEnviando(false);
    }
  }

  if (feito) {
    return (
      <Tela>
        <Titulo sub="O nó conferiu e espalhou a transação. Ela entra no próximo bloco minerado.">Enviado</Titulo>
        <Cartao>
          <Linha rotulo="Valor" valor={`${textoDeUnidades(conferido.unidades)} HYX`} />
          <Linha rotulo="Para" valor={destino.trim()} mono />
          <Linha rotulo="Transação" valor={feito} mono />
        </Cartao>
        <Botao titulo="Pronto" grande aoTocar={() => router.back()} />
      </Tela>
    );
  }

  if (revisando) {
    return (
      <Tela>
        <Titulo sub="Confira com calma. Uma transação enviada não volta.">Revise</Titulo>
        <Cartao>
          <SeloTeste rede={rede} />
          <Linha rotulo="Valor" valor={`${textoDeUnidades(conferido.unidades)} HYX`} />
          <Linha rotulo="Taxa" valor={`${textoDeUnidades(taxa)} HYX`} />
          <Linha rotulo="Total" valor={`${textoDeUnidades(conferido.unidades + taxa)} HYX`} />
          <Linha rotulo="Para" valor={destino.trim()} mono />
        </Cartao>
        {erro ? <Aviso tipo="falha">{erro}</Aviso> : null}
        <Botao titulo="Confirmar e enviar" grande ocupado={enviando} aoTocar={confirmar} />
        <Botao titulo="Corrigir" tipo="leve" aoTocar={() => setRevisando(false)} desligado={enviando} />
      </Tela>
    );
  }

  return (
    <Tela>
      <Titulo sub={c.conta ? `Gastável: ${textoDeUnidades(c.conta.saldo)} HYX` : undefined}>Enviar HYX</Titulo>
      <Campo
        rotulo="Para (endereço)"
        value={destino}
        onChangeText={setDestino}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder={`${endereco.prefixo(rede)}1…`}
        erro={conferido.erroDestino}
      />
      <View style={{ flexDirection: "row", gap: 10 }}>
        <View style={{ flex: 1 }}>
          <Botao titulo="Escanear QR" tipo="leve" aoTocar={() => router.push("/escanear")} />
        </View>
        <View style={{ flex: 1 }}>
          <Botao titulo="Colar" tipo="leve" aoTocar={async () => setDestino((await Clipboard.getStringAsync()).trim())} />
        </View>
      </View>
      <Campo rotulo="Valor (HYX)" value={valor} onChangeText={setValor} keyboardType="decimal-pad" placeholder="0,00" erro={conferido.erroValor} />
      {c.conta ? (
        <Botao titulo="Usar tudo" tipo="leve" aoTocar={() => setValor(textoDeUnidades(c.conta!.saldo > taxa ? c.conta!.saldo - taxa : 0n, 0).replace(/\./g, ""))} />
      ) : null}
      {!c.info ? <Aviso tipo="atencao">Conecte um nó em Ajustes para enviar.</Aviso> : null}
      <Texto fraco pequeno>Taxa desta rede: {textoDeUnidades(taxa)} HYX.</Texto>
      <Botao titulo="Revisar" grande desligado={!conferido.pronto || !c.info} aoTocar={() => setRevisando(true)} />
    </Tela>
  );
}
