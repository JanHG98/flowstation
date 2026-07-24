# LXC-Deployment

## Empfehlung

Die KMF sollte bereits im Testaufbau einen eigenen LXC erhalten:

- Debian 13 oder kompatibel,
- dediziertes Management-VLAN,
- keine Portweiterleitung aus Benutzer- oder Funknetzen,
- verschlüsselter Proxmox-Storage soweit verfügbar,
- restriktive Backup-Ziele,
- Zeitquelle mit stabiler Uhr.

## Ports

```text
TCP 8190  WebUI und Management-/Edge-API im Open-Lab-Modus
```

## Installation

```bash
sudo system-backend/kmf/install/install.sh
```

## Rechte

```text
/etc/netcore/kmf.toml          root:netcore-kmf 0640
/var/lib/netcore-kmf           netcore-kmf      0700
master.key                     netcore-kmf      0600
vault.json                     netcore-kmf      0600
bootstrap/*.json               netcore-kmf      0600
```

Der systemd-Dienst verwendet `UMask=0077`, `NoNewPrivileges`, leere Capability-Sets und ein schreibgeschütztes System mit genau einem freigegebenen Datenverzeichnis.
