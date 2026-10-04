import { router } from "expo-router";
import { useEffect, useState } from "react";
import { Alert, Text } from "react-native";

import { useCarteira } from "../estado/carteira";
import { Aviso, Botao, Campo, Cartao, Linha, Tela, Texto, Titulo } from "../ui/componentes";
import { useTema } from "../ui/tema";

export default function Ajustes() {
  const t = useTema();
  const c = useCarteira();
  const [no, setNo] = useState(c.noEndereco ?? "");
  const [testando, setTestando] = useState(false);
  const [resultado, setResultado] = useState<{ ok: boolean; texto: string } | null>(null);

  useEffect(() => {
    if (c.noEndereco) setNo(c.noEndereco);
  }, [c.noEndereco]);

  async function conectar() {
    setTestando(true);
    setResultado(null);
    try {
      const i = await c.definirNo(no);
      setResultado({ ok: true, texto: `Conectado: ${i.rede}, bloco ${i.altura.toLocaleString("pt-BR")}, ${i.pares} conexões, programa ${i.versao}.` });
      c.atualizar();
    } catch (e: any) {
      setResultado({ ok: false, texto: String(e?.message ?? e) });
    } finally {
      setTestando(false);
    }
  }

  function apagar() {
    Alert.alert(
      "Tirar a carteira deste aparelho?",
      "O saldo continua na rede, mas só volta para quem tiver a cópia de segurança e a senha. Sem elas, ele fica perdido para sempre.",
      [
        { text: "Cancelar", style: "cancel" },
        {
          text: "Tirar",
          style: "destructive",
          onPress: async () => {
            if (!(await c.confirmarDono("Confirme para tirar a carteira"))) return;
            await c.apagar();
            router.replace("/boas-vindas");
          },
        },
      ],
    );
  }

  return (
    <Tela>
      <Titulo>Ajustes</Titulo>
      <Cartao>
        <Text style={{ color: t.tinta, fontSize: 17, fontWeight: "600" }}>Nó</Text>
        <Texto fraco pequeno>
          No PC, ligue Ajustes → Carteiras de celular no programa Hyurax e use o IP dele na rede (por exemplo 192.168.0.10). A conexão não é cifrada: use em rede de confiança.
        </Texto>
        <Campo rotulo="Endereço do nó" value={no} onChangeText={setNo} autoCapitalize="none" autoCorrect={false} keyboardType="url" placeholder="192.168.0.10:8800" />
        {resultado ? <Aviso tipo={resultado.ok ? "ok" : "falha"}>{resultado.texto}</Aviso> : null}
        <Botao titulo="Conectar" ocupado={testando} desligado={!no.trim()} aoTocar={conectar} />
      </Cartao>
      <Cartao>
        <Text style={{ color: t.tinta, fontSize: 17, fontWeight: "600" }}>Carteira</Text>
        <Linha rotulo="Endereço" valor={c.enderecoTexto ?? "—"} mono />
        <Botao titulo="Cópia de segurança" tipo="leve" aoTocar={() => router.push("/backup")} />
        <Botao titulo="Tirar a carteira deste aparelho" tipo="perigo" aoTocar={apagar} />
      </Cartao>
      <Cartao>
        <Text style={{ color: t.tinta, fontSize: 17, fontWeight: "600" }}>Sobre</Text>
        <Texto fraco pequeno>
          Hyurax, aplicativo de carteira. Esta é a rede de TESTE: o HYX não tem valor, não há venda nem promessa de ganho. O código é aberto (MIT ou Apache-2.0).
        </Texto>
      </Cartao>
    </Tela>
  );
}
