# HYURAX Compute (V1): o servidor da plataforma num contêiner.
#
# Não compila nada aqui: baixa o pacote oficial da Release (Linux, x86_64) e
# confere a soma SHA-256 publicada junto. O diário (o banco da plataforma)
# fica em /dados: monte ali um DISCO QUE NÃO SE APAGA. Sem disco persistente,
# cada reinício apaga contas, saldos e tarefas. Ver docs/OPERAR-A-PLATAFORMA.md.

FROM alpine:3.20 AS baixar
ARG VERSAO=v1.5.1
RUN apk add --no-cache curl
WORKDIR /baixar
RUN set -eu; \
    pacote="hyurax-${VERSAO}-linux-x86_64"; \
    base="https://github.com/NOXqubit/hyurax/releases/download/${VERSAO}"; \
    curl -fsSLO "${base}/${pacote}.tar.gz"; \
    curl -fsSLO "${base}/${pacote}.tar.gz.sha256"; \
    sha256sum -c "${pacote}.tar.gz.sha256"; \
    tar xzf "${pacote}.tar.gz"; \
    install -m 0755 "${pacote}/hyurax-plataforma" /hyurax-plataforma

FROM alpine:3.20
RUN adduser -D -h /dados hyurax && mkdir -p /dados/plataforma && chown -R hyurax /dados
COPY --from=baixar /hyurax-plataforma /usr/local/bin/hyurax-plataforma
USER hyurax
WORKDIR /dados
# O provedor de hospedagem dá a porta em $PORT e termina o HTTPS na frente:
# HYX_HTTPS=1 liga o cookie Secure e o HSTS; HYX_PROXY=1 lê o IP do cliente no
# X-Forwarded-For (um salto). HYX_PUBLICO é o endereço que aparece para os
# agentes (troque pelo seu). HYX_ADMIN_EMAIL cria o administrador na primeira
# vez e mostra a senha provisória uma vez no log.
ENV PORT=10000 HYX_PASTA=/dados/plataforma HYX_HTTPS=1 HYX_PROXY=1
EXPOSE 10000
HEALTHCHECK --interval=30s --timeout=5s CMD wget -qO- "http://127.0.0.1:${PORT}/saude" || exit 1
CMD ["hyurax-plataforma", "servir"]
