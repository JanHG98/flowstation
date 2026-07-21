# NetCore Directory

NetCore Directory ist der zentrale Namens- und Metadatendienst für die Basisstation. Der Dienst läuft als Python-Anwendung mit SQLite-Datenbank und stellt Weboberfläche sowie HTTP-API bereit.

## Datenbereiche

- Geräte
- Basisstationen
- Gruppen
- Gerätegruppen bzw. Statusgruppen
- Statusmeldungen

## Installation

```bash
sudo useradd --system --home /opt/netcore-directory --shell /usr/sbin/nologin netcore-directory
sudo install -d -o netcore-directory -g netcore-directory /opt/netcore-directory
sudo install -d -o netcore-directory -g netcore-directory /var/lib/netcore-directory
sudo install -m 0755 system-backend/directory/netcore-directory.py \
  /opt/netcore-directory/netcore-directory.py
```

Wichtig: Dateiname in `ExecStart` und tatsächlich installierter Python-Dateiname müssen identisch sein. Bei älteren Servicevorlagen gab es unterschiedliche Namen; das vor dem Aktivieren prüfen.

Beispiel für die Basisstationsanbindung:

```toml
[netcore_directory]
enabled = true
base_url = "http://<DIRECTORY-IP>:8095"
timeout_ms = 2000
```

## Laufzeitverhalten

Die Basisstation fragt Directory-Daten für Anzeige und Statuslogik ab. Der Funkbetrieb ist nicht vollständig vom Directory abhängig. Bei Ausfall bleiben numerische IDs und lokal bekannte Zustände nutzbar.

Statusgruppen werden regelmäßig neu geladen. Änderungen an Mitgliedern können dadurch ohne Neustart der Basisstation wirksam werden. Bei erneuter Registrierung eines Gruppenmitglieds kann der zuletzt bekannte Status erneut ausgesendet werden.

## Optionale Exporte

Die Directory-Konfiguration unterstützt neben der reinen Namensauflösung optionale Laufzeit-Exporte, zum Beispiel:

- Präsenz
- Status
- CDR/Rufereignisse
- Notfälle
- Health
- SDS-Aktivität
- Positionen

Diese Optionen sollten nur aktiviert werden, wenn der Directory-Server die jeweilige Verarbeitung unterstützt.

## Sicherung

Die SQLite-Datenbank muss regelmäßig gesichert werden. Vor Importen oder größeren Strukturänderungen:

```bash
sudo systemctl stop netcore-directory.service
sudo cp /var/lib/netcore-directory/netcore-directory.db \
  /var/lib/netcore-directory/netcore-directory.db.$(date +%Y%m%d-%H%M%S).bak
sudo systemctl start netcore-directory.service
```

Pfad und Dateiname können je nach lokaler Unit abweichen.

## Health-Prüfung

```bash
curl -fsS http://127.0.0.1:8095/api/health
sudo systemctl status netcore-directory.service --no-pager
sudo journalctl -u netcore-directory.service -n 200 --no-pager
```
