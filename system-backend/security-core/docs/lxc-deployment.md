# LXC-Deployment

Empfehlung: Debian 13 LXC, 1–2 vCPU, 512 MiB RAM, 4 GiB Disk, eigener Management-VLAN.

```bash
sudo system-backend/security-core/install/install.sh
```

Danach:

```bash
systemctl status netcore-security-core
curl http://127.0.0.1:8180/health/ready
```

Persistente Pfade:

- `/etc/netcore/security-core.toml`
- `/var/lib/netcore-security-core/state.json`
- `/var/lib/netcore-security-core/lab-auth.seed`

Der Seed-Pfad benötigt Besitzer `netcore-security:netcore-security` und Modus `0600`.
