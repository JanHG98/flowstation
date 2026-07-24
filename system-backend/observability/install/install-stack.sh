#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
[[ ${EUID} -eq 0 ]] || { echo "install-stack.sh must run as root" >&2; exit 1; }
install -d -m 0755 /etc/prometheus/rules /etc/alertmanager /etc/loki /etc/promtail /etc/grafana/provisioning/datasources /etc/grafana/provisioning/dashboards /var/lib/grafana/dashboards
install -m 0644 "${ROOT}/system-backend/observability/stack/prometheus/prometheus.yml" /etc/prometheus/prometheus.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/prometheus/rules/netcore.rules.yml" /etc/prometheus/rules/netcore.rules.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/alertmanager/alertmanager.yml" /etc/alertmanager/alertmanager.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/loki/loki.yml" /etc/loki/loki.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/promtail/promtail.yml" /etc/promtail/promtail.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/grafana/provisioning/datasources/netcore.yml" /etc/grafana/provisioning/datasources/netcore.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/grafana/provisioning/dashboards/netcore.yml" /etc/grafana/provisioning/dashboards/netcore.yml
install -m 0644 "${ROOT}/system-backend/observability/stack/grafana/dashboards/netcore-overview.json" /var/lib/grafana/dashboards/netcore-overview.json
for unit in prometheus alertmanager loki promtail grafana-server; do
  if systemctl list-unit-files "${unit}.service" >/dev/null 2>&1; then systemctl enable --now "${unit}.service" || true; else echo "${unit}: no installed systemd unit, configuration staged only"; fi
done
echo "Stack configuration installed. No third-party binary was downloaded by this script."
