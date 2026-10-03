# API externa: mandar JOBs para um nó Hyurax

Documento Mestre §14 (Compute-as-a-Service), §17 e fase 5. Implementada em
`crates/hyurax-nucleo/src/api/externa.rs` e `contas.rs`; testada de ponta a
ponta em `crates/hyurax-nucleo/tests/api_externa.rs`.

> Rede de **teste**. Créditos de computação são contabilidade, não dinheiro
> nem HYX. Não há cobrança: quem roda o nó decide quem tem conta e quanto
> cada conta pode consumir.

## Quem faz o quê

| Quem | O que faz |
|---|---|
| Dono do nó | Liga a API (Ajustes → API externa, ou `hyurax-no painel --api-externa`), cria contas, dá a chave a cada cliente, revoga. |
| Cliente | Manda JOBs, acompanha, baixa relatórios e dados brutos, pela chave da conta dele. |

## Contas (dono do nó)

```bash
hyurax-no contas criar --nome "Laboratório X" --creditos 50000
```

```bash
hyurax-no contas listar
```

```bash
hyurax-no contas revogar --id 1
```

A chave aparece **uma vez**, na criação; em disco fica só o SHA-512 dela
(`contas.txt`, na pasta de configuração). A revogação vale na hora, mesmo
com o programa aberto. Os mesmos comandos existem na tela, em Ajustes.

## Uso (cliente)

Toda chamada leva `Authorization: Bearer CHAVE`. Os campos do JOB vão como
formulário (`application/x-www-form-urlencoded`).

| Método | Rota | O que faz |
|---|---|---|
| GET | `/api/v1/externa/conta` | limite, consumo e saldo de créditos |
| POST | `/api/v1/externa/estimar` | custo e tempo previstos |
| POST | `/api/v1/externa/jobs` | submete um JOB |
| GET | `/api/v1/externa/jobs` | os JOBs da conta, com estado e consumo |
| GET | `/api/v1/externa/jobs/ID` | um JOB inteiro |
| GET | `/api/v1/externa/jobs/ID/relatorio.json` (`.csv`, `.pdf`) | o relatório |
| GET | `/api/v1/externa/jobs/ID/unidades` | o registro de cada unidade (JSON por linha) |
| POST | `/api/v1/externa/jobs/ID/cancelar` | cancela |

Campos de um JOB: `dominio` (código), `tipo` (motor), `tamanho`, `passos`,
`parametros` (lista com vírgulas), `unidades`, `nivel` (1 a 5),
`redundancia`, `prazo_s`, `orcamento_milicreditos` (0 = o saldo inteiro),
`descricao`. Os códigos estão em [ULTRAX.md](ULTRAX.md) e na tela Ciência.

Exemplo (matriz 64 × 64, duas unidades, reexecução local):

```bash
curl -s -H "Authorization: Bearer $CHAVE" -d "dominio=7&tipo=1&tamanho=64&unidades=2&nivel=2" http://IP-DO-NO:8800/api/v1/externa/jobs
```

## Regras

- **Saldo.** Cada JOB recebe orçamento igual ao saldo da conta (ou menos,
  se pedir menos) e para sozinho quando chegar nele. O saldo é o limite
  menos o consumo dos JOBs da conta, lido do próprio JOB. As unidades que
  já estavam em voo quando o orçamento acabou ainda contam: o consumo pode
  passar um pouco do limite.
- **Limites.** 120 pedidos por minuto e 8 JOBs abertos por conta.
- **Isolamento.** Um JOB de outra conta responde 404, como se não
  existisse. A chave de conta não abre o resto do painel (estado, carteira,
  comandos).
- **Rede.** Com a API ligada, o nó aceita conexões de fora só nas rotas
  `/api/v1/externa/`; o resto do painel continua só deste computador (ou só
  leitura, com "ver no celular").

## Riscos e o que falta

- **Sem TLS.** A chave e os resultados passam em texto pela rede. Use só em
  rede local confiável, ou atrás de um túnel/proxy com HTTPS. TLS no próprio
  nó: PENDENTE (exige biblioteca de TLS, que hoje não entra por regra do
  projeto: nada com código em C).
- **Sem cobrança nem pagamento.** Ver [ECONOMIA.md](ECONOMIA.md).
- **Backend central (contas na nuvem, painel web multi-cliente):** não
  existe. Cada nó é o seu próprio backend; um serviço central exigiria
  servidor sempre no ar, que este projeto ainda não tem.
