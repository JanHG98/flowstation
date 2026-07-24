# LXC-Deployment

## Mindestanforderungen

- Debian/Ubuntu-LXC
- Rust-Toolchain für Installation aus dem Repo
- ausgehender HTTP/HTTPS-Zugriff zu aktivierten Connectoren
- Managementnetz-Zugriff auf TCP 8220
- lokaler oder erreichbarer Piper-Dienst für TTS

## Installation

```bash
sudo system-backend/application-gateway/install/install.sh
```

Danach:

```bash
systemctl status netcore-application-gateway --no-pager
curl -fsS http://127.0.0.1:8220/health/ready
```

## Netzgrenze

Die systemd-Unit benötigt nur `AF_UNIX`, `AF_INET` und `AF_INET6`. Keine TUN-/TAP-, GPIO-, SDR- oder Raw-Socket-Capabilities sind erforderlich.

## Daten

```text
/etc/netcore/application-gateway.toml
/var/lib/netcore-application-gateway/state.json
/var/lib/netcore-application-gateway/secrets.json
/var/lib/netcore-application-gateway/spool/
/var/lib/netcore-application-gateway/backups/
```

`secrets.json` muss getrennt von normalen State-Backups behandelt werden.
