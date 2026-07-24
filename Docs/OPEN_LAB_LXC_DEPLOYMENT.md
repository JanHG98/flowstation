# Open-Lab-LXC-Deployment

## Netzmodell

Jeder deploybare Backend-Dienst läuft vorzugsweise in einem eigenen unprivilegierten Debian-LXC mit statischer Adresse im isolierten Management-VLAN. Die Beispielbelegung `10.0.20.10` bis `10.0.20.26` steht in `deploy/open-lab/inventory.example.toml` und ist vor der Installation anzupassen.

Keine der Management-WebUIs darf in dieser Phase aus dem Internet oder einem untrusted Client-VLAN erreichbar sein. Open Lab bedeutet nicht „halbwegs sicher“, sondern ausdrücklich „jeder erreichbare Client darf verwalten“.

## Reihenfolge

Der Deployer berechnet die Reihenfolge aus `depends_on`. Damit werden Node Gateway und autoritative Fachdienste vor Control Room und Observability installiert. Ein Dienst kann gezielt ausgewählt werden; seine transitiven Abhängigkeiten werden automatisch ergänzt.

## Befehle

```bash
python3 deploy/open-lab/netcore-deploy.py validate
python3 deploy/open-lab/netcore-deploy.py plan
python3 deploy/open-lab/netcore-deploy.py render
python3 deploy/open-lab/netcore-deploy.py apply --dry-run
python3 deploy/open-lab/netcore-deploy.py apply
python3 deploy/open-lab/netcore-deploy.py status
```

## Besondere Container

- IP Gateway: `/dev/net/tun` und NET_ADMIN im eigenen Namespace; Beispiel unter `deploy/open-lab/lxc/ip-gateway.conf.example`.
- Recorder/Media Library: NFS-Mount getrennt vorbereiten; Live-State bleibt lokal, Archiv darf auf NFS liegen.
- Security Core/KMF: eigenes restriktives Management-Segment wird trotz Open Lab dringend empfohlen.
- Observability: Zugriff auf alle `/metrics`, `/health/live` und `/health/ready`-Endpunkte.

## Rollback

Der Deployer überschreibt keine Fach-State-Dateien. Vor einem Update bleiben die dienstspezifischen Backup-Funktionen maßgeblich. Quellstand und gerenderte Konfiguration sind im Deployment-Bundle reproduzierbar; ein Rollback erfolgt durch erneutes Anwenden des vorherigen Bundles plus Wiederherstellung der jeweiligen Dienst-Backups.
