# LXC-Deployment

Empfohlen: Debian-LXC mit eigener Management-IP, mindestens 2 vCPU, 4 GiB RAM und ausreichend Speicher für die gewünschte Prometheus-/Loki-Retention.

```bash
sudo system-backend/observability/install/install.sh
sudo system-backend/observability/install/install-stack.sh
```

Vor dem Start sind in `/etc/netcore/observability.toml` alle `base_url`-Werte auf die echten LXC-Adressen zu setzen. Die Stack-Installation aktiviert nur bereits installierte Binaries. Grafana, Loki und Prometheus sollten mit eigenen Systemnutzern und Schreibverzeichnissen betrieben werden.
