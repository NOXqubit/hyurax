#!/usr/bin/env bash
# Instala um nó semente da rede de TESTE do Hyurax num servidor Linux.
#
#   curl -fsSLO https://raw.githubusercontent.com/NOXqubit/hyurax/main/deploy/instalar-semente.sh
#   less instalar-semente.sh          # leia antes de rodar
#   sudo bash instalar-semente.sh
#
# O que faz, e só isso:
#   1. baixa o hyurax-no da Release e CONFERE a soma SHA-256 (para se não bater);
#   2. cria o usuário de sistema "hyurax", sem login, dono de /var/lib/hyurax;
#   3. instala dois serviços systemd que ligam sozinhos: o nó (porta 8790) e o
#      explorador de blocos estático (porta 8080, só leitura);
#   4. abre as duas portas no firewall local (ufw ou firewalld, se existirem).
#
# Não mexe em SSH, não cria carteira e não minera. A rede é de teste: HYX não
# tem valor.
set -euo pipefail

VERSAO="${HYURAX_VERSAO:-v1.1.0}"
PORTA_NO=8790
PORTA_WEB=8080
DADOS=/var/lib/hyurax
BASE="https://github.com/NOXqubit/hyurax/releases/download/${VERSAO}"
FONTE="https://raw.githubusercontent.com/NOXqubit/hyurax/main/deploy"

[ "$(id -u)" -eq 0 ] || { echo "rode com sudo"; exit 1; }
case "$(uname -m)" in
  x86_64) PACOTE="hyurax-${VERSAO}-linux-x86_64" ;;
  aarch64|arm64) PACOTE="hyurax-${VERSAO}-linux-arm64-e-celular" ;;
  *) echo "arquitetura não suportada: $(uname -m)"; exit 1 ;;
esac
for cmd in curl tar sha256sum python3 systemctl; do
  command -v "$cmd" >/dev/null || { echo "falta o programa: $cmd"; exit 1; }
done

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
cd "$TMP"
echo "== baixando ${PACOTE}"
curl -fsSLO "${BASE}/${PACOTE}.tar.gz"
curl -fsSLO "${BASE}/${PACOTE}.tar.gz.sha256"
echo "== conferindo SHA-256"
sha256sum -c "${PACOTE}.tar.gz.sha256"
tar xzf "${PACOTE}.tar.gz"
install -m 0755 "${PACOTE}/hyurax-no" /usr/local/bin/hyurax-no

echo "== usuário e pastas"
id hyurax >/dev/null 2>&1 || useradd --system --home-dir "$DADOS" --shell /usr/sbin/nologin hyurax
install -d -o hyurax -g hyurax -m 0750 "$DADOS" "$DADOS/testnet"
install -d -o hyurax -g hyurax -m 0755 "$DADOS/publico"
curl -fsSL "${FONTE}/explorador/index.html" -o "$DADOS/publico/index.html"
chown hyurax:hyurax "$DADOS/publico/index.html"

echo "== serviços"
curl -fsSL "${FONTE}/hyurax-semente.service" -o /etc/systemd/system/hyurax-semente.service
curl -fsSL "${FONTE}/hyurax-explorador.service" -o /etc/systemd/system/hyurax-explorador.service
systemctl daemon-reload
systemctl enable --now hyurax-semente.service hyurax-explorador.service

echo "== firewall local"
if command -v ufw >/dev/null; then ufw allow "${PORTA_NO}/tcp"; ufw allow "${PORTA_WEB}/tcp"; fi
if command -v firewall-cmd >/dev/null; then
  firewall-cmd --permanent --add-port="${PORTA_NO}/tcp" --add-port="${PORTA_WEB}/tcp" && firewall-cmd --reload
fi
# As imagens Ubuntu da Oracle Cloud vêm sem ufw e com uma regra REJECT no
# iptables: sem isto, a porta fica fechada mesmo liberada no painel da nuvem.
if ! command -v ufw >/dev/null && ! command -v firewall-cmd >/dev/null && command -v iptables >/dev/null; then
  for p in "${PORTA_NO}" "${PORTA_WEB}"; do
    iptables -C INPUT -p tcp --dport "$p" -m state --state NEW -j ACCEPT 2>/dev/null       || iptables -I INPUT -p tcp --dport "$p" -m state --state NEW -j ACCEPT
  done
  if command -v netfilter-persistent >/dev/null; then netfilter-persistent save; fi
fi

sleep 3
systemctl --no-pager --lines=5 status hyurax-semente.service || true
IP="$(curl -fsS https://api.ipify.org || echo SEU_IP)"
cat <<FIM

Pronto. Falta liberar as portas ${PORTA_NO} e ${PORTA_WEB} (TCP, entrada) no painel
do provedor de nuvem (lista de segurança / grupo de segurança).

  semente:     ${IP}:${PORTA_NO}
  explorador:  http://${IP}:${PORTA_WEB}/
  registro:    journalctl -u hyurax-semente -f
  identidade:  sudo -u hyurax cat ${DADOS}/testnet/no.chave | grep publica

Teste de outro computador:
  hyurax-no no --rede testnet --pasta teste --semente ${IP}:${PORTA_NO}
FIM
