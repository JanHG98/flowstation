# Konfiguration

Die Basisstation nutzt TOML. Der aktuelle Parser erwartet:

```toml
config_version = "0.6"
stack_mode = "Bs"
```

Die reale Konfiguration enthält standortspezifische und teilweise geheime Daten. Beispiele in diesem Wiki verwenden daher ausschließlich Platzhalter.

## Wichtige Sektionen

| Sektion | Zweck |
|---|---|
| `[phy_io]` / `[phy_io.soapysdr]` | SDR-Backend, Gerät, Sample-Rate, Verstärkung und Center-Frequenzen |
| `[net_info]` | MCC und MNC |
| `[cell_info]` | Carrier, Duplex, Zellkennung, Dienste und Rufparameter |
| `[cell_info.sds_command_control]` | autorisierte U-STATUS-Systembefehle |
| `[recovery]` | Cache-Replay und Re-Attract unbekannter Endgeräte |
| `[health]` | Zustandsüberwachung und optionaler Watchdog |
| `[dashboard]` | Bind-Adresse, Port, Anmeldung und Update-Arbeitsverzeichnis |
| `[recording]` | lokale Sprachaufzeichnung und Archivierung |
| `[audio_player]` | WAV-/MP3-Bibliothek und Funkaussendung |
| `[tts]` | Piper-Endpunkt, Stimmen, Vorlagen und Cache |
| `[netcore_directory]` | Directory-Anbindung |
| `[control_room]` | Verbindung zur Leitstelle |
| `[asterisk]`, `[brew]`, `[telegram_alerts]`, `[wx_service]` | optionale Integrationen |

## Minimales Gerüst

```toml
config_version = "0.6"
stack_mode = "Bs"

[phy_io]
backend = "SoapySdr"

[phy_io.soapysdr]
tx_freq = <DOWNLINK-HZ>
rx_freq = <UPLINK-HZ>
device = "driver=<TREIBER>"
sample_rate = 600000
tx_center_freq = <TX-CENTER-HZ>
rx_center_freq = <RX-CENTER-HZ>

[net_info]
mcc = <MCC>
mnc = <MNC>

[cell_info]
freq_band = 4
main_carrier = <CARRIER>
duplex_spacing = 0
freq_offset = 0
reverse_operation = false
location_area = 1
colour_code = 1
timezone = "Europe/Berlin"
registration = true
deregistration = true
voice_service = true

[dashboard]
bind = "0.0.0.0"
port = 8080
username = "<BENUTZER>"
password = "<LANGES-PASSWORT>"
```

Die genauen Feldnamen der SoapySDR-Sektion können vom Treiber abhängen. Die vorhandene Beispielkonfiguration im Repository ist die maßgebliche Vorlage für den eingesetzten Hardwarezweig.

## Zellparameter

### Carrier und Frequenz

`main_carrier` ist die TETRA-Carrier-Nummer. `secondary_carrier` aktiviert zusammen mit `dual_carrier_enabled` den zweiten Träger. Die tatsächlichen Frequenzen ergeben sich aus Band, Duplexspacing, Offset und Reverse-Betrieb.

Center-Frequenzen und Sample-Rate müssen den gesamten genutzten Bereich abdecken. Eine formal gültige Carrier-Konfiguration kann sonst trotzdem am Passband-Check scheitern.

### Zeit und Standort

`timezone` verwendet einen IANA-Namen, zum Beispiel `Europe/Berlin`. Damit kann die Basisstation UTC und lokalen Offset inklusive Sommerzeit senden.

### Rufverhalten

Wichtige optionale Parameter:

- `hangtime_secs` – Offenhaltezeit eines Gruppenrufs nach Ende der Aussendung.
- `call_timeout_secs` – maximale Rufdauer; `0` bedeutet ohne Zeitlimit.
- `ul_inactivity_secs` – Abbruch einer Senderphase ohne Uplink-Sprachrahmen.
- `periodic_registration_secs` – Intervall für periodische Re-Registrierung; `0` deaktiviert.
- `release_group_on_same_speaker_retake` – Workaround für ältere Geräte bei erneutem PTT in der Hangtime.

## Recovery

`[recovery]` unterscheidet zwei Mechanismen:

- **Proaktives Replay (`enabled`)**: bekannte Endgeräte werden nach Start anhand des Caches erneut angesprochen.
- **Reaktives Re-Attract (`reactive_enabled`)**: ein unbekannt auftauchendes Endgerät wird zur Registrierung aufgefordert.

Beides sollte zunächst konservativ getestet werden, besonders bei heterogenen Gerätegenerationen.

## Health

Der Health-Monitor ist beobachtend und kann Core-Liveness, Backhaul, registrierte Geräte und Überlastung zusammenfassen. Der optionale automatische Neustart bei Core-Stall ist ein RF-wirksamer Eingriff und sollte nur bewusst aktiviert werden.

## Zugangsdaten

Folgende Werte niemals veröffentlichen:

- Dashboard-Passwort
- Telegram-Bot-Token und Chat-IDs
- Brew-/Asterisk-Zugangsdaten
- Directory- oder Control-Room-Tokens
- ActionURL-Token

Dateirechte:

```bash
chmod 600 /etc/netcore/config.toml
```

## Prüfung vor Neustart

```bash
cp /etc/netcore/config.toml /etc/netcore/config.toml.pre-change
sudo systemctl restart tetra.service
sudo journalctl -u tetra.service -n 200 --no-pager
```

Bei einem Parserfehler nicht mehrfach neu starten, sondern die erste konkrete Fehlermeldung korrigieren. Danach prüfen, ob versehentlich die Fallback-Konfiguration aktiv wurde.
