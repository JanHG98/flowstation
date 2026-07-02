# NetCore Control Room Operator Profiles – vollständige Einbauanleitung

Datei: `netcore-control-room-operator-profiles-v1-files.zip`

Dieser Stand baut auf dem funktionierenden RBAC/Auth-Stand auf. Er ändert vor allem den nativen Operator-Client und ergänzt lokale Profile.

## 0. Was dieser Stand bringt

- lokales Operator-Profil statt ständig `--api` und `--token`
- optional `token_file`, damit Tokens nicht in der Shell-History landen
- Standard-Node, z. B. `SRV-M_TBS-01`
- Standard-Operator-ID, z. B. `jan`
- mehrere Profile über `--profile`
- bessere API-Fehlerausgaben mit Statuscode und Server-Body
- systemd-Unit enthält jetzt direkt `EnvironmentFile=-/etc/netcore-control-room/control-room.env`
- keine Patch-Dateien, nur komplette Dateien

## 1. Vorher Status prüfen

Auf dem Control-Room-LXC:

```bash
cd /opt/netcore/flowstation
systemctl status netcore-control-room --no-pager
curl -i http://127.0.0.1:9010/health
```

Optional Backup:

```bash
cp -a /opt/netcore/flowstation /opt/netcore/flowstation.backup.$(date +%Y%m%d-%H%M%S)
cp -a /etc/netcore-control-room /etc/netcore-control-room.backup.$(date +%Y%m%d-%H%M%S)
cp -a /var/lib/netcore-control-room /var/lib/netcore-control-room.backup.$(date +%Y%m%d-%H%M%S)
```

## 2. ZIP einspielen

```bash
cd /opt/netcore/flowstation
unzip -o /pfad/zu/netcore-control-room-operator-profiles-v1-files.zip
```

Beispiel, falls die Datei in `/tmp` liegt:

```bash
cd /opt/netcore/flowstation
unzip -o /tmp/netcore-control-room-operator-profiles-v1-files.zip
```

## 3. Bauen

Im LXC nur Control Room und Operator bauen:

```bash
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator
```

Wichtig: Im LXC weiterhin nicht `bluestation-bs` bauen.

## 4. systemd-Unit aktualisieren

Der neue Service-File-Stand enthält die Env-Datei direkt. Installieren:

```bash
install -m 0644 \
  system-backend/control-room/systemd/netcore-control-room.service \
  /etc/systemd/system/netcore-control-room.service
```

Falls noch ein alter, kaputter Drop-in-Override existiert, anschauen:

```bash
systemctl cat netcore-control-room
```

Wenn du weiterhin einen Drop-in unter `/etc/systemd/system/netcore-control-room.service.d/override.conf` hast und er nur `EnvironmentFile`/`ExecStart` enthält, kannst du ihn entfernen, weil der Hauptservice das jetzt selbst kann:

```bash
rm -f /etc/systemd/system/netcore-control-room.service.d/override.conf
rmdir --ignore-fail-on-non-empty /etc/systemd/system/netcore-control-room.service.d
```

Dann:

```bash
systemctl daemon-reload
systemctl restart netcore-control-room
journalctl -u netcore-control-room -n 80 --no-pager
```

Erwartung:

```text
SQLite persistence enabled ...
NetCore Control Room listening bind=0.0.0.0:9010 ...
```

## 5. Auth nochmal kurz gegenprüfen

Health ohne Token darf gehen:

```bash
curl -i http://127.0.0.1:9010/health
```

API ohne Token muss abgewiesen werden:

```bash
curl -i http://127.0.0.1:9010/api/overview
```

Erwartung:

```text
HTTP/1.1 401 Unauthorized
```

## 6. Operator-Profil systemweit anlegen

Wenn du deinen Admin- oder Operator-Token direkt in der Config speichern willst:

```bash
./target/release/netcore-control-room-operator profiles init \
  --system \
  --profile default \
  --api http://10.0.1.25:9010 \
  --default-node SRV-M_TBS-01 \
  --operator-id jan
```

Dann Datei öffnen:

```bash
nano /etc/netcore-control-room/operator.toml
chmod 600 /etc/netcore-control-room/operator.toml
```

Dort eintragen:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token = "HIER_DEIN_OPERATOR_ODER_ADMIN_TOKEN"
```

Besser für später: Admin-Token nur für Admin, Operator-Token für Alltag.

## 7. Sicherere Variante mit token_file

Token-Datei anlegen:

```bash
install -m 600 -o root -g root /dev/null /etc/netcore-control-room/operator.token
nano /etc/netcore-control-room/operator.token
```

In diese Datei nur den Token schreiben, ohne Anführungszeichen.

Dann Profil erzeugen:

```bash
./target/release/netcore-control-room-operator profiles init \
  --system \
  --force \
  --profile default \
  --api http://10.0.1.25:9010 \
  --token-file /etc/netcore-control-room/operator.token \
  --default-node SRV-M_TBS-01 \
  --operator-id jan
```

## 8. Profil prüfen

```bash
./target/release/netcore-control-room-operator profiles show
```

Erwartung ungefähr:

```json
{
  "config_path": "/etc/netcore-control-room/operator.toml",
  "profile": "default",
  "api": "http://10.0.1.25:9010",
  "token_present": true,
  "token_source": "profile default token_file /etc/netcore-control-room/operator.token",
  "default_node": "SRV-M_TBS-01",
  "operator_id": "jan"
}
```

Der Klartext-Token wird bewusst nicht ausgegeben.

## 9. Ab jetzt kurze Befehle nutzen

Vorher:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  overview
```

Jetzt:

```bash
./target/release/netcore-control-room-operator overview
```

Dashboard:

```bash
./target/release/netcore-control-room-operator dashboard
```

Subscriber:

```bash
./target/release/netcore-control-room-operator subscribers --online
```

## 10. Commands mit Profil testen

Weil `default_node` und `operator_id` im Profil stehen, reicht jetzt:

```bash
./target/release/netcore-control-room-operator kick --issi 2010002
```

DGNA Attach:

```bash
./target/release/netcore-control-room-operator dgna --issi 2020004 --gssi 15205
```

DGNA Detach:

```bash
./target/release/netcore-control-room-operator dgna --issi 2020004 --gssi 15205 --detach
```

Emergency clear:

```bash
./target/release/netcore-control-room-operator clear-emergency
```

Manuell überschreiben geht weiterhin:

```bash
./target/release/netcore-control-room-operator \
  kick --node SRV-M_TBS-01 --issi 2010002 --operator jan
```

## 11. Mehrere Profile

Beispiel:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = "/etc/netcore-control-room/operator.token"

[profiles.event]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "event-lst"
token_file = "/etc/netcore-control-room/event-operator.token"
```

Nutzung:

```bash
./target/release/netcore-control-room-operator --profile event dashboard
./target/release/netcore-control-room-operator --profile event kick --issi 2010002
```

## 12. Fehlerbild prüfen

Mülltoken soll jetzt mit lesbarem Fehler kommen:

```bash
./target/release/netcore-control-room-operator \
  --token "bla bla bla" \
  overview
```

Erwartung sinngemäß:

```text
Control Room API returned 401 Unauthorized ...
```

## 13. Rollback

Wenn etwas schiefgeht:

```bash
systemctl stop netcore-control-room
rm -rf /opt/netcore/flowstation
cp -a /opt/netcore/flowstation.backup.YYYYMMDD-HHMMSS /opt/netcore/flowstation
```

Bei Bedarf auch Config/DB zurück:

```bash
rm -rf /etc/netcore-control-room
cp -a /etc/netcore-control-room.backup.YYYYMMDD-HHMMSS /etc/netcore-control-room

rm -rf /var/lib/netcore-control-room
cp -a /var/lib/netcore-control-room.backup.YYYYMMDD-HHMMSS /var/lib/netcore-control-room
```

Dann:

```bash
systemctl daemon-reload
systemctl start netcore-control-room
```

## 14. Geänderter Dateiumfang

Der ZIP enthält wieder komplette Dateien, keine Patch-Dateien. Wichtig neu/geändert:

- `system-backend/control-room/operator/src/main.rs`
- `system-backend/control-room/operator/README.md`
- `system-backend/control-room/config/operator.example.toml`
- `system-backend/control-room/docs/operator-profiles.md`
- `system-backend/control-room/systemd/netcore-control-room.service`
- kompletter bisheriger RBAC/Auth-Dateistand bleibt enthalten
