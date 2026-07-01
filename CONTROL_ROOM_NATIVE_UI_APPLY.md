# NetCore Control Room Native UI v1 – vollständige Einbauanleitung

Datei: `netcore-control-room-native-ui-v1-files.zip`

Dieser Stand ergänzt das bisherige Control-Room-System um eine native Desktop-UI:

- keine Web-App
- keine Browser-Oberfläche
- eigenständig ausführbares Rust/egui-Programm
- nutzt dieselbe `operator.toml`/`token_file`-Logik wie die Operator-CLI
- spricht per HTTP/API mit dem Control-Room-Core auf dem LXC

## 0. Was ist wichtig?

Bisher gebaut:

- Control Room Core auf dem LXC
- SQLite-Persistenz
- Node-Anbindung der TBS
- Token-Auth/RBAC
- Operator-CLI als Admin-/Diagnosewerkzeug

Neu in diesem ZIP:

- `system-backend/control-room/ui` als native Desktop-App

Die UI ist ein Operator-Client. Sie ersetzt nicht den Core-Service auf dem LXC.

---

## 1. ZIP ins Repo einspielen

Auf dem System, auf dem dein Repo liegt:

```bash
cd /opt/netcore/flowstation
unzip -o /pfad/zu/netcore-control-room-native-ui-v1-files.zip
```

Oder, falls die ZIP in `/tmp` liegt:

```bash
cd /opt/netcore/flowstation
unzip -o /tmp/netcore-control-room-native-ui-v1-files.zip
```

Dieses ZIP enthält komplette Dateien und keine Patch-Dateien.

---

## 2. LXC/Core prüfen

Auf dem Control-Room-LXC:

```bash
systemctl status netcore-control-room --no-pager -l
curl -i http://127.0.0.1:9010/health
curl -i http://127.0.0.1:9010/api/overview
```

Erwartung:

```text
/health -> 200 OK
/api/overview ohne Token -> 401 Unauthorized
```

Mit echtem Token:

```bash
source /etc/netcore-control-room/control-room.env

./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  overview
```

Erwartung: JSON-Übersicht kommt.

---

## 3. UI auf einem Linux-Operator-PC bauen

Die UI ist für einen Operator-PC gedacht, nicht zwingend für den headless LXC.

Pakete installieren:

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config curl ca-certificates \
  libx11-dev libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libasound2-dev libudev-dev libfontconfig1-dev
```

Rust installieren, falls noch nicht vorhanden:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Dann im Repo:

```bash
cd /opt/netcore/flowstation

cargo build --release \
  --manifest-path system-backend/control-room/ui/Cargo.toml
```

Das Binary liegt danach hier:

```text
system-backend/control-room/ui/target/release/netcore-control-room-ui
```

Start:

```bash
./system-backend/control-room/ui/target/release/netcore-control-room-ui
```

---

## 4. Operator-Profil für die UI anlegen

Empfohlen: Token nicht direkt in der TOML speichern, sondern in einer Token-Datei.

```bash
mkdir -p ~/.config/netcore/control-room
install -m 600 /dev/null ~/.config/netcore/control-room/operator.token
nano ~/.config/netcore/control-room/operator.token
```

In die Datei kommt nur der Operator- oder Admin-Token, ohne Anführungszeichen.

Dann Config schreiben:

```bash
cat > ~/.config/netcore/control-room/operator.toml <<'NETCORE_OPERATOR_TOML'
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = "/home/jan/.config/netcore/control-room/operator.token"
NETCORE_OPERATOR_TOML

chmod 600 ~/.config/netcore/control-room/operator.toml
```

Passe den `token_file`-Pfad an deinen Benutzer an, falls du nicht `jan` bist.

UI starten:

```bash
./system-backend/control-room/ui/target/release/netcore-control-room-ui
```

---

## 5. UI mit explizitem Profil starten

```bash
./system-backend/control-room/ui/target/release/netcore-control-room-ui \
  --profile default
```

Oder mit expliziter Config:

```bash
./system-backend/control-room/ui/target/release/netcore-control-room-ui \
  --config ~/.config/netcore/control-room/operator.toml \
  --profile default
```

Oder ohne Config nur mit Token-Datei:

```bash
./system-backend/control-room/ui/target/release/netcore-control-room-ui \
  --api http://10.0.1.25:9010 \
  --token-file ~/.config/netcore/control-room/operator.token
```

---

## 6. Windows-Build

Auf Windows:

1. Rust installieren
2. Repo/ZIP einspielen
3. PowerShell im Repo öffnen

Build:

```powershell
cargo build --release --manifest-path system-backend/control-room/ui/Cargo.toml
```

Binary:

```text
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
```

Config-Beispiel:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = "C:\\Users\\Jan\\AppData\\Roaming\\netcore\\control-room\\operator.token"
```

Start:

```powershell
.\system-backend\control-room\ui\target\release\netcore-control-room-ui.exe
```

---

## 7. Was die UI v1 kann

Tabs:

- Übersicht
- Teilnehmer
- Gruppen
- Rufe
- SDS
- Standorte
- Commands/Audit
- Admin/Tokens
- Raw JSON

Befehle aus der Seitenleiste:

- Kick ISSI
- DGNA Attach/Detach
- Emergency Clear

Admin/Tokens:

- Tokens listen
- Token erstellen
- Token enable/disable
- Token löschen

Wichtig: Die Admin-Tokenverwaltung funktioniert nur mit Admin-Token. Mit Operator-Token zeigt die UI dort einen 403/401-Hinweis, der Rest funktioniert weiter.

---

## 8. Auth-Test mit UI

Wenn die UI Übersicht lädt, ist der Token gültig.

Wenn oben ein Fehler wie `401 Unauthorized` steht:

- Token fehlt
- Token-Datei falsch
- falsches Profil
- API-Adresse falsch

Prüfen:

```bash
cat ~/.config/netcore/control-room/operator.toml
cat ~/.config/netcore/control-room/operator.token
```

Dann CLI-Gegenprobe:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token-file ~/.config/netcore/control-room/operator.token \
  overview
```

---

## 9. Hinweise

- Die UI benutzt die bestehenden REST-Endpunkte des Control-Room-Core.
- Auf der TBS muss nichts geändert werden.
- Der LXC-Core muss laufen.
- `/health` bleibt öffentlich, `/api/*` bleibt geschützt.
- Die UI speichert keine Tokens selbst, außer du trägst `token = "..."` direkt in `operator.toml` ein.
- Sicherer ist `token_file = "..."`.

---

## 10. Rollback

Da die UI additiv ist, ist Rollback simpel:

```bash
rm -rf /opt/netcore/flowstation/system-backend/control-room/ui
```

Der Control-Room-Core läuft davon unabhängig weiter.
