# Backup und Fallback

## Primär- und Fallback-Konfiguration

Kann die angegebene Primärkonfiguration nicht geladen werden, versucht die Basisstation automatisch eine Datei mit angehängtem `.fallback`:

```text
config.toml
config.toml.fallback
```

Wird Fallback verwendet, zeigt das Dashboard dauerhaft eine rote Warnung. Das System kann damit weiterlaufen, aber die Ursache der fehlerhaften Primärdatei muss behoben werden.

## Dashboard-Sicherung

Vor dem Überschreiben durch den Konfigurationseditor wird zusätzlich eine `.bak`-Datei erzeugt. Diese ist eine kurzfristige Rückfallebene, ersetzt aber kein externes Backup.

## Zu sichernde Daten

- `config.toml`
- `config.toml.fallback`
- Konfigurations-`.bak`
- Directory-SQLite-Datenbank
- Directory-Exportdatei
- TTS-Vorlagen
- lokale Medienbibliothek
- Recovery-Cache
- Systemd-Units und Environment-Dateien
- bei Bedarf Aufzeichnungen und JSON-Metadaten

## Backup-Beispiel

```bash
sudo systemctl stop tetra.service
sudo tar -C / -czf /var/backups/netcore-$(date +%Y%m%d-%H%M%S).tar.gz \
  etc/netcore \
  var/lib/netcore \
  var/lib/netcore-directory
sudo systemctl start tetra.service
```

Pfade an die reale Installation anpassen. Große Aufnahmeverzeichnisse gegebenenfalls separat sichern.

## Wiederherstellung

1. Dienst stoppen.
2. beschädigte Dateien separat wegkopieren.
3. Backup entpacken oder einzelne Dateien wiederherstellen.
4. Eigentümer und Dateirechte prüfen.
5. Basisstation manuell mit der wiederhergestellten Konfiguration starten.
6. erst danach Systemd wieder aktivieren.

## Nach einem Fallback-Start

```bash
sudo journalctl -u tetra.service -b --no-pager | grep -iE 'config|fallback|parse|error'
diff -u /etc/netcore/config.toml.fallback /etc/netcore/config.toml
```

Nicht die Fallback-Datei blind über die Primärdatei kopieren. Sie kann bewusst konservativ oder älter sein.
