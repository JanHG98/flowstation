# LXC-Deployment

Empfohlen ist ein Debian-LXC mit eigener IP im isolierten SwMI-Testnetz.

```bash
sudo system-backend/sds-router/install/install.sh
```

Danach:

```bash
systemctl status netcore-sds-router
journalctl -u netcore-sds-router -f
```

Pfade:

```text
/usr/local/bin/netcore-sds-router
/etc/netcore/sds-router.toml
/var/lib/netcore-sds-router/messages.json
/etc/systemd/system/netcore-sds-router.service
```

Firewall im Testnetz:

- eingehend TCP 8150 nur aus Management-/Entwicklungsnetz,
- ausgehend zum Node Gateway TCP 8080,
- kein Internetzugriff erforderlich.

Für jede TBS, die den zentralen Router verwenden soll, muss `central_sds_routing = true` im Abschnitt `[control_room]` gesetzt und die TBS neu gestartet werden.
