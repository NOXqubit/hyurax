# Operar a plataforma HYURAX Compute

O servidor é um binário só, `hyurax-plataforma`, que vem nos pacotes de
terminal de cada versão (Linux, Windows, Mac). Ele serve os painéis, a API
`/api/v1/` e fala com os agentes. O que ele sabe fica numa pasta de dados:

| Na pasta | O que é |
|---|---|
| `diario.hyx` | **o banco**: cada evento numa linha, com hash encadeado. Nunca edite à mão |
| `diario.trava` | trava de uma instância só (o sistema solta se o processo cair) |
| `resultados/` | os resultados aceitos, por tarefa (apagados 30 dias depois do fim) |
| `entregas/` | entregas esperando conferência |
| `copias/` | cópias conferidas do diário, a cada 6 horas (as 28 mais novas) |

## Experimentar no próprio computador

```bash
hyurax-plataforma servir --pasta plataforma-teste --porta 8790 --teste
```

Abra `http://127.0.0.1:8790`. Com `--teste`, o pagamento confirma na hora e
todas as telas avisam que é ambiente de teste. Para ser administrador:

```bash
hyurax-plataforma admin --pasta plataforma-teste --email voce@exemplo.com
```

(com o servidor parado; a senha aparece uma vez). Para emprestar o próprio
computador: no painel, crie a conta de fornecedor, vá em **Computadores →
Adicionar computador** e cole o código no programa Hyurax, tela **Fornecer**
(endereço `http://127.0.0.1:8790`).

## Hospedar de verdade

O diário é o banco, então **o disco precisa ficar**. O plano grátis do Render
apaga o disco a cada reinício: serve para a semente da rede de teste, não
para a plataforma.

Opções que funcionam:

- **Render** com disco persistente (plano pago com "Disk"): serviço Docker
  apontando para `deploy/plataforma/Dockerfile`, disco montado em `/dados`.
- **Qualquer VPS** (Hetzner, Contabo, Oracle, Magalu Cloud...): o mesmo
  contêiner, ou o binário com o `systemd`, atrás de um proxy com HTTPS
  (Caddy, Nginx).

Variáveis de ambiente (o contêiner já traz as três primeiras):

| Variável | Para quê |
|---|---|
| `HYX_PASTA=/dados/plataforma` | a pasta de dados (no disco persistente) |
| `HYX_HTTPS=1` | cookie `Secure` e HSTS (o HTTPS termina no proxy) |
| `HYX_PROXY=1` | quantos proxies confiáveis há na frente (o IP do cliente vem do `X-Forwarded-For`) |
| `PORT` | a porta que o provedor dá |
| `HYX_PUBLICO=https://compute.seu-dominio` | o endereço que aparece para os agentes |
| `HYX_ADMIN_EMAIL=voce@...` | só na primeira vez: cria o administrador e mostra a senha **uma vez no log**. Troque a senha logo depois e tire a variável |
| `HYX_TESTE=1` | ambiente de teste (nunca em produção) |

Depois de subir:

1. Entre com o administrador e **troque a senha** (Minha conta).
2. **Configuração:** a chave Pix e o titular da empresa (sem isso, o
   pagamento avisa que a venda não abriu), preços, parte do fornecedor,
   retenção, limites e versão mínima do agente.
3. **Registro:** confira que os eventos estão entrando.
4. Guarde o **selo do diário** (Painel) fora do servidor de vez em quando.

## Rotina

- **Pagamentos (Pix manual):** o cliente gera o pedido com um código
  (`HYX-P-000123`) e paga na chave da empresa com o código na mensagem. No
  painel, **Pagamentos → Confirmar** com a referência do Pix recebido e a
  taxa do banco. O saldo entra na hora.
- **Saques:** o ganho fica retido pelos dias da configuração, libera
  sozinho, e o fornecedor pede o saque com a chave Pix dele. **Saques →
  Aprovar**, pague pelo banco da empresa, **Pago** com a referência.
- **Estorno (chargeback):** **Pagamentos → Estornar**. Conta que fica
  negativa é bloqueada e os ganhos retidos das tarefas dela ficam
  segurados até alguém olhar (depois da análise, `POST /api/v1/admin/tarefas/N/soltar-ganhos` libera).
- **Devolução de saldo** (direito de arrependimento de 7 dias, ou pedido
  depois): **Usuários → Ajuste** com valor negativo e o motivo, e devolva o
  valor por Pix pelo banco da empresa.
- **Termos e privacidade:** as páginas `/termos.html` e `/privacidade.html`
  vêm como rascunho para revisão de advogado. Antes de abrir a venda, troque o
  texto revisado em `crates/hyurax-plataforma/web/` e publique uma versão nova.
  O **contato do suporte** (Configuração) aparece no rodapé.
- **Alertas:** força bruta, muitas contas do mesmo IP, resultado falso,
  hardware repetido, pedido de agente repetido, estorno e padrões de fraude.
  Resolver com uma nota do que foi feito.

## Kill switch

| Nível | Onde |
|---|---|
| **KILL TASK** | Todas as tarefas → Parar |
| **KILL NODE** | Nós → Quarentena, Manutenção ou Banir |
| **DISABLE WORKLOAD TYPE** | Configuração → desmarcar o motor (as tarefas dele param) |
| **EMERGENCY COMPUTE STOP** | Painel → Parada de emergência: nada é despachado e tudo que roda para no próximo pulso dos agentes |

## Cópias e recuperação

- Cópia automática a cada 6 horas em `copias/`, conferida inteira antes de
  valer. Baixe uma cópia pelo painel (**Registro → Baixar cópia do diário**)
  ou pela linha de comando, com o servidor rodando:

  ```bash
  hyurax-plataforma copia --pasta /dados/plataforma --destino diario-hoje.hyx
  ```

- Conferir um diário (corrente de hash, livro fechando em zero, contagens):

  ```bash
  hyurax-plataforma conferir --pasta /dados/plataforma
  ```

- **Queda:** o servidor relê o diário ao subir; a última linha incompleta (luz
  que acabou no meio da escrita) é cortada, e as unidades que estavam com os
  agentes continuam valendo pelo prazo gravado.
- **Diário adulterado** (linha mudada ou apagada no meio): o servidor não
  abre e diz a linha. Restaure a última cópia boa de `copias/`.
- **Modo de leitura:** se um evento gravado não aplicar (não deveria
  acontecer), a plataforma para de aceitar escrita até ser reaberta, e o
  painel mostra.

## Atualizar

Troque a versão no `Dockerfile` (`ARG VERSAO`) ou o binário, e reinicie. O
diário é o mesmo de uma versão para outra. Para obrigar os fornecedores a
atualizar o programa, suba a **versão mínima do agente** na Configuração: nó
abaixo dela não recebe trabalho e vê o aviso na tela Fornecer.
