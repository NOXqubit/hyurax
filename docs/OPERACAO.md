# Operação: monitoramento, cópias, incidentes e implantação gradual

Documento Mestre, fase 10 (produção). O que existe para operar um nó
Hyurax / Ultrax hoje, e o que ainda falta antes de qualquer rede principal.
Rede de **teste**.

## 1. Monitoramento

### Saúde do nó

`GET http://127.0.0.1:8800/api/v1/saude` (público como o resumo, sem
endereço nem saldo):

```json
{"ok":false,"alertas":["nenhum par conectado"],"versao":"1.0.0","rede":"hyurax-testnet",
 "ligado_ha_s":5230,"altura":1842,"ultimo_bloco_ha_s":95,"pares":0,
 "minerando":true,"ultrax_ligado":true,"jobs_abertos":1,"api_externa":false}
```

`ok` é falso quando: nenhum par conectado; o último bloco tem mais de dez
intervalos (20 min na rede de teste); o programa está trancado. Um
monitor externo (cron, Zabbix, Uptime Kuma) só precisa olhar `ok`.

```bash
curl -s http://127.0.0.1:8800/api/v1/saude
```

### Onde mais olhar

| O quê | Onde |
|---|---|
| Registro do programa (com teto e rotação) | `%LOCALAPPDATA%\Hyurax\registros\hyurax.log` |
| Eventos de cada JOB | `ciencia/jobs/ID/eventos.jsonl` |
| Unidades e evidências | `unidades.jsonl`, `consenso.jsonl` do JOB |
| Histórico do ULTRAX (auditável) | `hyurax-no ultrax auditar` |
| Memória, handles e threads ao longo do tempo | `scripts/teste-longo.ps1` (CSV) |

## 2. Cópias de segurança e recuperação de desastre

```bash
hyurax-no copia criar --destino E:\copias
```

```bash
hyurax-no copia conferir --origem E:\copias\hyurax-copia-1791058393
```

```bash
hyurax-no copia restaurar --origem E:\copias\hyurax-copia-1791058393
```

- A cópia leva as duas pastas (configuração e dados), menos a chave de
  sessão, a trava, o cache do navegador e os registros, com um manifesto
  SHA-512 de cada arquivo.
- **Restaurar** confere a cópia inteira antes, só roda com o programa
  fechado e nunca apaga: o que existia vira `.antes-da-restauracao-<unix>`.
- **A cópia leva segredos**: a carteira (cifrada; a senha continua
  valendo), o segundo fator e a identidade do nó (em texto). Guarde como
  guardaria a carteira, fora deste computador.
- A cadeia se refaz sozinha pela rede; a carteira, não. Sem cópia nem frase
  de recuperação, carteira perdida é saldo perdido.

### Cenários

| Aconteceu | Faça |
|---|---|
| Disco morreu | Instale de novo; `copia restaurar` da última cópia; a cadeia sincroniza sozinha. |
| Cadeia corrompida | Feche o programa e apague o arquivo `.cadeia`: o nó baixa de novo dos pares. |
| JOB parou no meio (queda de energia) | Abra o programa: o JOB volta do último checkpoint (SHA-512 conferido, cópia anterior se o último estiver estragado). |
| Identidade do nó vazou | `hyurax-no identidade girar` com o programa fechado. A antiga fica ao lado; não há revogação anunciada na rede (PENDENTE). |
| Chave de conta da API externa vazou | `hyurax-no contas revogar --id N` (vale na hora). |
| Segredo de lançamento vazou | Grave: quem tem o segredo publica atualização aceita por todas as instalações. Gere outra chave, publique uma versão com a chave pública nova por canal manual (site e Releases), avise os usuários. |

## 3. Resposta a incidentes

1. **Conter.** Desligar o que expõe: API externa, "ver no celular", ou o
   nó inteiro. Revogar chaves de conta e girar a identidade se houver sinal
   de vazamento.
2. **Preservar.** Copiar (`copia criar`) antes de mexer: os registros, o
   histórico do ULTRAX e as evidências dos JOBs são a prova.
3. **Entender.** `hyurax-no ultrax auditar`; ler `hyurax.log` e os
   `eventos.jsonl` do período.
4. **Corrigir e testar.** Um teste que reproduz a falha entra junto com a
   correção.
5. **Comunicar.** Falha grave: relato privado do GitHub (ver
   [SECURITY.md](../SECURITY.md)); depois da correção, nota pública com o
   que aconteceu, o alcance e o que mudou.

## 4. Implantação gradual

A ordem é a do [ROTEIRO-LANCAMENTO.md](ROTEIRO-LANCAMENTO.md): GitHub →
rede de teste pública → rede principal. Dentro de cada versão:

1. **CI verde**: testes, clippy, os cenários de rede, os três nós e o
   `cargo audit`.
2. **Ensaio local**: instalação limpa (`scripts/testar-instalacao.ps1`) e
   execução longa (`scripts/teste-longo.ps1`, horas, não minutos).
3. **Canário**: a versão nova vai primeiro para as máquinas do dono
   (atualização assinada, passo 2b de [LANCAR-A-REDE.md](LANCAR-A-REDE.md)),
   um dia rodando.
4. **Todos**: só então o `atualizacao.txt` assinado vai para a Release
   que os programas instalados olham.
5. **Volta atrás**: o instalador da versão anterior continua nas Releases;
   reinstalar por cima mantém os dados (pastas do usuário separadas).

## 5. Auditorias

| Auditoria | Estado |
|---|---|
| Auditoria interna contra o Documento Mestre | feita: [AUDITORIA-DOCUMENTO-MESTRE.md](AUDITORIA-DOCUMENTO-MESTRE.md) |
| Modelo de ameaças | feito: [MODELO-DE-AMEACAS.md](MODELO-DE-AMEACAS.md) |
| Vulnerabilidades conhecidas nas dependências | automática no CI (`cargo audit`) |
| Auditoria externa de segurança | **PENDENTE**, obrigatória antes de qualquer rede principal |
| Parecer jurídico | **PENDENTE**, obrigatório antes de qualquer coisa com dinheiro real ([ECONOMIA.md](ECONOMIA.md)) |
| Assinatura Authenticode do `.exe` | **PENDENTE** (certificado do dono) |
