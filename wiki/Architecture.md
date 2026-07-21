# Architektur

## Gesamtbild

Die NetCore-Architektur trennt RF-nahe Funktionen, Bedienoberfläche und optionale zentrale Dienste. Dadurch kann die Basisstation auch ohne Directory oder Leitstelle lokal weiterarbeiten.

```text
TETRA-Endgeräte
      │ RF
      ▼
┌───────────────────────────────┐
│ Basisstation: bluestation-bs  │
│ PHY · MAC · MLE · CMCE · SDS  │
│ Recorder · Audio · Dashboard  │
└───────┬─────────┬─────────────┘
        │         │
        │ HTTP    │ WebSocket/HTTP
        ▼         ▼
 NetCore Directory       NetCore Control Room
        │
        ├── Geräte- und Gruppennamen
        ├── Statusmeldungen und Statusgruppen
        └── optionale Laufzeit-Exporte

Optionale Dienste:
Asterisk · Brew · Telegram · WX · NetCore Piper · NFS
```

## Komponenten im Repository

| Bereich | Pfad | Aufgabe |
|---|---|---|
| Basisstations-Binärdatei | `bins/bluestation-bs` | Start, Konfigurationsladung und Zusammenschaltung der Entitäten |
| Protokollkern | `crates/tetra-core`, `tetra-saps`, `tetra-pdus` | TETRA-Zeitbasis, SAPs und PDU-Strukturen |
| Laufzeitentitäten | `crates/tetra-entities` | RF, Registrierung, Rufe, SDS, Dashboard und Integrationen |
| Konfiguration | `crates/tetra-config` | TOML-Parsing, Validierung und Defaults |
| Leitstellenkern | `bins/netcore-control-room` | Zentrale Zustands- und Befehlsinstanz |
| Operator-Werkzeug | `system-backend/control-room/operator` | CLI-/Dashboard-Zugriff auf die Leitstelle |
| Native Leitstellen-UI | `system-backend/control-room/ui` | Eigenständige Bedienoberfläche |
| Directory | `system-backend/directory` | SQLite-Verzeichnis und HTTP-API |
| TTS-Dienst | `system-backend/tts` | Piper-basierte WAV-Erzeugung |

## Datenflüsse

### Luftschnittstelle

Der PHY-Backend-Treiber liefert Uplink-Samples und erhält Downlink-Samples. Die Protokollschichten verarbeiten Registrierung, Gruppenbindungen, Rufe, SDS und Statusmeldungen. Laufzeitinformationen werden an Dashboard, Recorder, Directory-Export und Leitstelle gespiegelt.

### Dashboard

Das Dashboard läuft im Prozess der Basisstation. Es zeigt den aktuellen Zustand, schreibt Konfigurationsänderungen und kann einen kontrollierten Neustart oder Updatevorgang anstoßen. Es ist daher eine administrative Oberfläche und sollte nicht ungeschützt aus fremden Netzen erreichbar sein.

### Directory

Directory-Daten sind ergänzende Metadaten. Fällt der Dienst aus, bleiben Funkbetrieb und lokale numerische IDs grundsätzlich verfügbar; lediglich Namen, Statusbezeichnungen und zentrale Gruppenzuordnungen können fehlen oder aus dem letzten Laufzeitstand stammen.

### Control Room

Eine Basisstation verbindet sich als Node mit der Leitstelle. Die Leitstelle sammelt Zustand und Ereignisse und kann autorisierte Befehle zurücksenden. Die lokale Basisstation bleibt die RF-Instanz; die Leitstelle ersetzt nicht den lokalen Protokollkern.

## Ausfallgrenzen

| Ausfall | Erwartetes Verhalten |
|---|---|
| Directory nicht erreichbar | Funkbetrieb läuft lokal weiter; Bezeichnungen/Policies können eingeschränkt sein |
| Leitstelle nicht erreichbar | Basisstation arbeitet weiter und versucht die Verbindung erneut |
| NFS nicht erreichbar | Lokale Aufnahme/Medien können weiterlaufen; Archivkopien werden später erneut versucht |
| Piper nicht erreichbar | Vorhandene WAV-Dateien bleiben nutzbar; neue TTS-Erzeugung schlägt fehl |
| Dashboard nicht erreichbar | RF-Dienst kann weiterlaufen; Bedienung erfolgt über Systemd und Dateien |
| Primärkonfiguration ungültig | Automatischer Versuch mit `<config>.fallback`; deutliche Dashboard-Warnung |
