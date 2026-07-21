# Fehlersuche

## Vorgehensweise

1. Fehlerzeitpunkt und betroffene ISSI/GSSI notieren.
2. Dienstzustand und ersten Fehler im Journal prüfen.
3. Konfiguration gegen Fallback und letzte Sicherung vergleichen.
4. RF-, Protokoll- und Integrationsfehler getrennt betrachten.
5. Nur eine Variable gleichzeitig ändern.
6. Nach einem Fix den vollständigen Ablauf einschließlich Release erneut testen.

## Dienst startet nicht

```bash
sudo systemctl status tetra.service --no-pager
sudo journalctl -u tetra.service -b -n 300 --no-pager
/usr/local/bin/bluestation-bs /etc/netcore/config.toml
```

Typische Ursachen:

- TOML-Fehler oder falsche `config_version`
- SDR nicht gefunden
- Sample-Rate nicht unterstützt
- Dual-Carrier-Passband ungültig
- Port bereits belegt
- fehlende native Bibliothek
- Verzeichnis nicht beschreibbar

## Dashboard nicht erreichbar

```bash
ss -ltnp | grep -E ':8080|bluestation'
curl -I http://127.0.0.1:8080/
```

Bind-Adresse, Port, Firewall und Auth-Konfiguration prüfen. Nach UI-Updates Browser hart neu laden.

## Funkgerät registriert nicht

- MCC/MNC, Frequenz, Colour Code und Location Area prüfen.
- Downlink-Signal und Uplink-Empfang getrennt betrachten.
- Gerätelogs auf Location Update prüfen.
- Allowlist/Recovery-Regeln kontrollieren.
- Zeitbasis und SDR-Buffer beobachten.

## Gruppenruf ohne Sprache

- Affiliation vorhanden?
- Traffic-Carrier/Timeslot zugewiesen?
- Floor Grant erhalten?
- Uplink-Sprachrahmen sichtbar?
- Audio-Codec oder SDR-Pfad gestört?
- Hangtime-Retake eines älteren Geräts?

## Call bleibt hängen

- U-DISCONNECT/D-RELEASE im Log suchen.
- `ul_inactivity_secs` prüfen.
- AudioPlayer-Release-Guard abwarten.
- Carrier-/Timeslot-Anzeige mit dem tatsächlichen Scheduler vergleichen.
- anschließend gezielt Gerät re-registrieren, nicht sofort alle Teilnehmer kicken.

## Directory-Namen fehlen

```bash
curl -fsS http://<DIRECTORY-IP>:8095/api/health
sudo journalctl -u tetra.service -n 300 --no-pager | grep -i directory
```

Base-URL, Timeout und Firewall prüfen. Numerische IDs sollten trotzdem sichtbar bleiben.

## Audio/TTS gestört

Siehe [[Audio-Zentrale]]. Besonders `ffmpeg`, Piper-Endpunkt, Cache-Rechte, NFS-Mount und Ruf-Release prüfen.

## Nach Änderung schlimmer als vorher

- Basisstation stoppen.
- Primärkonfiguration sichern.
- bekannten Stand oder Fallback manuell testen.
- Clean-Build ausführen.
- erst danach wieder automatisch starten.
