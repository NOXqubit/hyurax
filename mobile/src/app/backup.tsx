import * as Clipboard from "expo-clipboard";
import { router, useLocalSearchParams } from "expo-router";
import { useState } from "react";
import { Share, Text } from "react-native";

import { useCarteira } from "../estado/carteira";
import { Aviso, Botao, Cartao, Tela, Texto, Titulo } from "../ui/componentes";
import { irParaInicio } from "../ui/navegar";
import { useTema } from "../ui/tema";

export default function Backup() {
  const t = useTema();
  const c = useCarteira();
  const { primeira } = useLocalSearchParams<{ primeira?: string }>();
  const [texto, setTexto] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  async function mostrar() {
    setErro(null);
    if (!(await c.confirmarDono("Confirme para ver a cópia de segurança"))) return setErro("Cancelado.");
    setTexto(await c.arquivoDeBackup());
  }

  return (
    <Tela>
      <Titulo sub="Se o celular quebrar ou sumir, é com esta cópia e a senha que o saldo volta, aqui ou no programa do PC.">Guarde uma cópia</Titulo>
      <Aviso tipo="atencao">
        A cópia está cifrada com a sua senha: sem a senha, ela não gasta nada. Mas sem a cópia E a senha, ninguém recupera o saldo, nem nós.
      </Aviso>
      {texto ? (
        <>
          <Cartao>
            <Text selectable style={{ color: t.tinta2, fontFamily: "monospace", fontSize: 11.5, lineHeight: 17 }}>
              {texto}
            </Text>
          </Cartao>
          <Botao titulo="Copiar" aoTocar={() => Clipboard.setStringAsync(texto)} />
          <Botao titulo="Salvar (compartilhar com um app de notas ou e-mail para você)" tipo="leve" aoTocar={() => Share.share({ message: texto })} />
          <Texto fraco pequeno>No PC, salve o conteúdo como carteira.txt e importe pela tela Carteira.</Texto>
        </>
      ) : (
        <Botao titulo="Mostrar a cópia" grande aoTocar={mostrar} />
      )}
      {erro ? <Aviso tipo="falha">{erro}</Aviso> : null}
      <Botao titulo="Feito, continuar" tipo="leve" aoTocar={() => (primeira || !router.canGoBack() ? irParaInicio() : router.back())} />
    </Tela>
  );
}
