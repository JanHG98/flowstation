# Systemd-Service

Eine robuste Unit startet die Basisstation mit einem festen Konfigurationspfad und einem dedizierten Benutzer.

## Beispiel

```ini
[Unit]
Description=NetCore TETRA Basisstation
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=netcore
Group=netcore
WorkingDirectory=/opt/netcore
ExecStart=/usr/local/bin/bluestation-bs /etc/netcore/config.toml
Restart=on-failure
RestartSec=5
TimeoutStopSec=20
LimitNOFILE=65536
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

Als `/etc/systemd/system/tetra.service` speichern.

## Aktivieren

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now tetra.service
sudo systemctl status tetra.service --no-pager
```

## Rechte

Der Dienstbenutzer benötigt Zugriff auf:

- SDR-Gerät bzw. USB-/Gerätegruppe
- Konfiguration
- lokale Aufnahme- und Medienverzeichnisse
- Cache-Verzeichnisse
- gemountete NFS-Ziele
- eventuell Netzwerkverwaltung, falls WLAN aus dem Dashboard gesteuert wird

Keine pauschalen Root-Rechte vergeben, wenn gezielte Gruppen- oder PolicyKit-Regeln ausreichen.

## Neustartverhalten

`Restart=on-failure` startet bei unerwartetem Prozessende neu, aber nicht bei einem sauberen Stop. Ein zu kurzer Neustartzyklus kann bei einer dauerhaft falschen RF-Konfiguration das Log fluten; fünf Sekunden sind ein sinnvoller Ausgangswert.

## Ergänzende Units

- `netcore-directory.service`
- `netcore-piper.service`
- `netcore-control-room.service`

NFS-Mounts sollten als eigene Mount-Unit oder über `/etc/fstab` mit netzwerktauglichen Optionen bereitgestellt werden. Die Basisstation darf nicht davon abhängen, dass ein langsames NFS den Start unbegrenzt blockiert.
