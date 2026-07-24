# LXC-Deployment

## Proxmox-Gerätedurchreichung

Für einen unprivilegierten LXC werden üblicherweise folgende Einträge benötigt:

```text
lxc.cgroup2.devices.allow: c 10:200 rwm
lxc.mount.entry: /dev/net/tun dev/net/tun none bind,create=file
```

Danach im Container prüfen:

```bash
ls -l /dev/net/tun
ip -V
nft --version
```

## Pakete

```bash
apt update
apt install -y iproute2 nftables ca-certificates
```

Der DNS- und Testserver ist eingebaut; `dnsmasq`, nginx und tcpdump sind nicht erforderlich.

## Installation

```bash
sudo system-backend/ip-gateway/install/install.sh
sudo editor /etc/netcore/ip-gateway.toml
```

Zunächst `mode = "shadow"` starten und in der WebUI den Kernel-Plan prüfen. Danach:

```toml
[interface]
mode = "authoritative"
```

```bash
sudo systemctl restart netcore-ip-gateway
journalctl -u netcore-ip-gateway -f
```

## Egress-Interface

`nat.egress_interface` muss dem tatsächlichen Interface des Containers entsprechen, zum Beispiel `eth0`. Ein falsch gesetztes Interface führt nicht zu heimlichem Fallback, sondern zu einem sichtbar fehlerhaften nftables-Reconcile.

## Rechte

Die systemd-Unit läuft als Benutzer `netcore` und erhält nur:

```text
CAP_NET_ADMIN
CAP_NET_RAW
CAP_NET_BIND_SERVICE
```

Diese Rechte sind für TUN, Routing/nftables und UDP/53 nötig. Der Container selbst muss die Capabilities ebenfalls erlauben.
