# Nó semente da rede de TESTE do Hyurax no Render (plano grátis, sem cartão).
#
# O Render só deixa entrar HTTPS. Por isso o nó escuta WebSocket na porta que o
# Render dá ($PORT) e os outros nós chegam por wss://NOME.onrender.com/p2p. Por
# dentro do WebSocket é a conexão de sempre (aperto Noise XX, validação de cada
# bloco). Ver docs/NO-SEMENTE.md.
#
# Não compila nada aqui (512 MB de memória não bastam para compilar Rust): baixa
# o programa oficial da Release e confere a soma SHA-256 publicada junto.
#
# O disco do plano grátis some a cada reinício: a cadeia é baixada de novo dos
# outros nós. A identidade do nó também muda a cada reinício, e tudo bem: uma
# semente é só uma porta de entrada.

FROM alpine:3.20 AS baixar
ARG VERSAO=v1.3.0
RUN apk add --no-cache curl
WORKDIR /baixar
RUN set -eu; \
    pacote="hyurax-${VERSAO}-linux-x86_64"; \
    base="https://github.com/NOXqubit/hyurax/releases/download/${VERSAO}"; \
    curl -fsSLO "${base}/${pacote}.tar.gz"; \
    curl -fsSLO "${base}/${pacote}.tar.gz.sha256"; \
    sha256sum -c "${pacote}.tar.gz.sha256"; \
    tar xzf "${pacote}.tar.gz"; \
    install -m 0755 "${pacote}/hyurax-no" /hyurax-no

FROM alpine:3.20
RUN adduser -D -h /dados hyurax
COPY --from=baixar /hyurax-no /usr/local/bin/hyurax-no
USER hyurax
WORKDIR /dados
ENV PORT=10000
EXPOSE 10000
# --porta 8790: a porta TCP de sempre, só dentro do contêiner (o Render não a
# abre para fora); --ws-porta $PORT: a que o Render publica em HTTPS.
CMD ["sh", "-c", "exec hyurax-no no --rede testnet --pasta /dados/no --porta 8790 --ws-porta \"${PORT}\" --sem-malha"]
