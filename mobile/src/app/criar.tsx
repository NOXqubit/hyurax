import { router } from "expo-router";
import { useState } from "react";

import { useCarteira } from "../estado/carteira";
import { Aviso, Botao, Campo, Tela, Texto, Titulo } from "../ui/componentes";

export default function Criar() {
  const c = useCarteira();
  const [senha, setSenha] = useState("");
  const [repetida, setRepetida] = useState("");
  const [progresso, setProgresso] = useState<number | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  async function criar() {
    setErro(null);
    if (senha !== repetida) return setErro("As duas senhas não são iguais.");
    try {
      setProgresso(0);
      await c.criar(senha, setProgresso);
      router.replace({ pathname: "/backup", params: { primeira: "1" } });
    } catch (e: any) {
      setErro(String(e?.message ?? e));
    } finally {
      setProgresso(null);
    }
  }

  return (
    <Tela>
      <Titulo sub="A senha protege a cópia de segurança da carteira. Ela não é guardada em lugar nenhum: sem ela, a cópia não abre.">Crie uma senha</Titulo>
      <Campo rotulo="Senha (pelo menos 10 caracteres)" value={senha} onChangeText={setSenha} secureTextEntry autoCapitalize="none" autoCorrect={false} textContentType="newPassword" />
      <Campo rotulo="A mesma senha de novo" value={repetida} onChangeText={setRepetida} secureTextEntry autoCapitalize="none" autoCorrect={false} />
      <Texto fraco pequeno>
        Uma frase comprida é mais forte e mais fácil de lembrar do que uma palavra cheia de símbolos.
      </Texto>
      {erro ? <Aviso tipo="falha">{erro}</Aviso> : null}
      <Botao
        titulo={progresso === null ? "Criar carteira" : `Protegendo… ${Math.round(progresso * 100)}%`}
        grande
        ocupado={progresso !== null}
        desligado={senha.length < 10}
        aoTocar={criar}
      />
    </Tela>
  );
}
