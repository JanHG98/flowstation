# NetCore-Basisstation

Dieses Wiki beschreibt den aktuellen Stand der **NetCore-Basisstation** und der zugehörigen Dienste. Es richtet sich an Betrieb, Entwicklung und Fehlersuche im eigenen TETRA-Labornetz.

> **Dokumentationsstand:** NetCore 1.3.0 · Konfigurationsformat 0.6 · NetCore Directory 0.2.0

## Was gehört zum System?

Die Basisstation besteht nicht nur aus dem RF-Prozess. Im aktuellen Ausbau gehören mehrere Bausteine zusammen:

- **Basisstation (`bluestation-bs`)** – TETRA-Luftschnittstelle, Registrierung, Gruppen- und Einzelrufe, SDS, U-STATUS und RF-Verarbeitung.
- **Web-Dashboard** – Bedienung, Überwachung, Konfiguration, Updates, Audio-Zentrale und Systemfunktionen.
- **NetCore Directory** – zentrale Bezeichnungen für Geräte, Basisstationen, Gruppen, Statusmeldungen und Statusgruppen.
- **NetCore Control Room** – optionale zentrale Leitstelle für mehrere Basisstationen.
- **NetCore Piper** – optionaler lokaler TTS-Dienst für deutschsprachige Sprachdateien.
- **Asterisk/Brew/Telegram/WX** – optionale Integrationen für Telefonie, Netzkopplung, Alarmierung und Wetterdaten.

## Schnellstart

1. [[Installation]] lesen und Systemabhängigkeiten installieren.
2. Eine geprüfte `config.toml` anlegen; siehe [[Configuration]].
3. Die Basisstation einmal manuell starten und RF-/SDR-Erkennung prüfen.
4. Anschließend den [[Systemd-Service]] einrichten.
5. Dashboard aufrufen und [[Backup-and-Fallback]] kontrollieren.

## Wichtige Betriebsregeln

- Frequenzen, Sendeleistung und Antennenaufbau müssen zum zulässigen Versuchsaufbau passen.
- Zugangsdaten, Tokens und Passwörter gehören nicht ins Repository oder Wiki.
- Vor Updates immer Konfiguration, Fallback-Datei, Directory-Datenbank und Medien sichern.
- Nach Änderungen an RF, Carrier oder Audio immer einen vollständigen Clean-Build durchführen.
- Dual Carrier benötigt passende RF-Bandbreite, Center-Frequenzen und Endgeräte-Konfiguration; siehe [[Dual-Carrier]].

## Begriffe

Im Wiki bezeichnet **NetCore** die gesamte Software- und Dienstlandschaft. **Basisstation** meint den konkreten TETRA-RF-Dienst bzw. das dazugehörige Gerät. Der technische Binärname `bluestation-bs` bleibt aus Kompatibilitätsgründen bestehen.
