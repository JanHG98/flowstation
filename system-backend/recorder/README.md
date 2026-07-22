# Recorder

## Zweck

Der Recorder zeichnet Sprachverkehr und zugehörige Metadaten zentral auf.

## Kernaufgaben

- Passive Aufnahme über Media-Taps
- Call, ISSI, GSSI, TBS und Sprecherwechsel zuordnen
- Notrufkennzeichnung, Integritätsprüfung und Retention
- Export und Archivierung auf zentralem Storage

## Architekturregel

Ein Recorder-Ausfall darf keinen laufenden Ruf beeinflussen.

## WebUI zur Verwaltung

Der Recorder erhält eine eigene Verwaltungsoberfläche für Aufnahmen, Suche und Aufbewahrung.

### Geplante Ansichten

- laufende und abgeschlossene Aufnahmen
- Suche nach Call, ISSI, GSSI, TBS und Zeitraum
- Sprecherwechsel, Notrufkennzeichnung und Metadaten
- Storage-Auslastung und Retention
- Integritätsstatus und Hashprüfung
- Export- und Löschaufträge

### Kritische Aktionen

- Aufnahme exportieren
- Aufbewahrungsfrist ändern
- Integritätsprüfung starten
- rechtlich zulässige Löschung auslösen

Ein Ausfall der WebUI darf die passive Aufnahme nicht unterbrechen.
