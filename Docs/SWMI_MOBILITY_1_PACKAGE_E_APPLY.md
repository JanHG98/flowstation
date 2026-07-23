# Einspielen – Mobility 1 Paket E

## 1. Bestehende Dienste stoppen

Auf TBS und Backend-LXC:

```bash
sudo systemctl stop tetra.service 2>/dev/null || true
sudo systemctl stop netcore-node-gateway.service 2>/dev/null || true
sudo systemctl stop netcore-mobility-core.service 2>/dev/null || true
```

## 2. Konfiguration sichern

```bash
sudo cp -a /etc/netcore /etc/netcore.backup-$(date +%Y%m%d-%H%M%S) 2>/dev/null || true
```

## 3. Altes Repository und Build-Artefakte entfernen

Das neue ZIP vollständig entpacken und als kompletten Repository-Stand verwenden. Danach:

```bash
cd /opt/netcore-tetra
rm -rf target
cargo clean
```

Damit können keine alten Binaries oder inkonsistenten Workspace-Artefakte gestartet werden.

## 4. Node Gateway aktualisieren

```bash
sudo system-backend/node-gateway/install/update.sh
```

Der Gateway enthält nun strukturierte Backend-Antworten mit `request_id` und `command_id`.

## 5. Mobility-Core-Konfiguration vorbereiten

```bash
sudo install -d /etc/netcore
sudo cp system-backend/mobility-core/config/mobility-core.example.toml \
  /etc/netcore/mobility-core.toml
sudo nano /etc/netcore/mobility-core.toml
```

Mindestens anpassen:

```toml
[node_gateway]
url = "ws://NODE-GATEWAY-IP:8080/ws/backend"
```

Es werden keine Tokens eingetragen.

## 6. Mobility Core installieren

```bash
sudo system-backend/mobility-core/install/install.sh
```

## 7. TBS neu bauen

Die TBS benötigt die neuen Mobility-Control-Kommandos:

```bash
rm -rf target
cargo clean
cargo test -p tetra-entities
cargo build --release --features asterisk
```

Anschließend wie bisher den gewünschten TBS-Dienst installieren beziehungsweise starten.

## 8. Kontrolle

```bash
sudo systemctl status netcore-node-gateway.service
sudo systemctl status netcore-mobility-core.service
sudo journalctl -u netcore-mobility-core.service -f
```

WebUI:

```text
http://MOBILITY-CORE-IP:8090/
```

## 9. Funktionstest

1. Zwei TBS am Node Gateway verbinden.
2. Ein Funkgerät auf TBS A registrieren.
3. In der Mobility-Core-WebUI den Teilnehmer prüfen.
4. Transfer von TBS A nach TBS B starten.
5. Phasen `export`, `import`, `source cleanup`, `completed` kontrollieren.
6. Teilnehmerlage und Gruppen auf TBS B prüfen.

## 10. Rollback

```bash
sudo systemctl stop netcore-mobility-core.service
sudo systemctl stop netcore-node-gateway.service
```

Vorherigen Repository-Stand und `/etc/netcore`-Sicherung wiederherstellen, anschließend alte Artefakte löschen und vollständig neu bauen.
