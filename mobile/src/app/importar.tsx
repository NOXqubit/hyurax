import * as Clipboard from "expo-clipboard";
import { useState } from "react";

import { useCarteira } from "../estado/carteira";
import { Aviso, Botao, Campo, Tela, Texto, Titulo } from "../ui/componentes";
import { irParaInicio } from "../ui/navegar";

export default function Importar() {
  const c = useCarteira();
  const [texto, setTexto] = useState("");
  const [senha, setSenha] = useState("");
  const [progresso, setProgresso] = useState<number | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  async function importar() {
    setErro(null);
    try {
      setProgresso(0);
      await c.importar(texto, senha, setProgresso);
      irParaInicio();
    } catch (e: any) {
      setErro(String(e?.message ?? e));
    } finally {
      setProgresso(null);
    }
  }

  return (
    <Tela>
      <Titulo sub="No PC, abra o arquivo carteira.txt (em %APPDATA%\Hyurax) num editor de texto, copie tudo e cole aqui. É o mesmo formato: a carteira passa a funcionar nos dois aparelhos.">
        Traga a sua carteira
      </Titulo>
      <Campo rotulo="Conteúdo do carteira.txt" value={texto} onChangeText={setTexto} multiline autoCapitalize="none" autoCorrect={false} placeholder="formato=HYURAX-CARTEIRA-v2…" />
      <Botao titulo="Colar da área de transferência" tipo="leve" aoTocar={async () => setTexto(await Clipboard.getStringAsync())} />
      <Campo rotulo="Senha da carteira" value={senha} onChangeText={setSenha} secureTextEntry autoCapitalize="none" autoCorrect={false} />
      <Texto fraco pequeno>Abrir leva alguns segundos de propósito: é o que torna caro adivinhar a senha de uma cópia roubada.</Texto>
      {erro ? <Aviso tipo="falha">{erro}</Aviso> : null}
      <Botao
        titulo={progresso === null ? "Importar" : `Abrindo… ${Math.round(progresso * 100)}%`}
        grande
        ocupado={progresso !== null}
        desligado={!texto.trim() || !senha}
        aoTocar={importar}
      />
    </Tela>
  );
}
