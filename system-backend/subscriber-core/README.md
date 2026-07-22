# Subscriber Core

## Zweck

Der Subscriber Core ist die zentrale Teilnehmerdatenbank des NetCore-TETRA-Netzes.

## Kernaufgaben

- ISSI, ITSI und Gerätezuordnungen verwalten
- Teilnehmernamen, Organisationen und Dienstprofile speichern
- Berechtigungen, Prioritäten und Sperrstatus bereitstellen
- Registrierungen freigeben oder ablehnen

## Abgrenzung

Aktuelle Zelle und Erreichbarkeit gehören zum Mobility Core; direkte Funk-PDUs werden hier nicht erzeugt.

## WebUI zur Verwaltung

Der Subscriber Core erhält eine eigene Verwaltungsoberfläche für Teilnehmer- und Geräte-Stammdaten.

### Geplante Ansichten

- Teilnehmer, ISSI/ITSI und zugeordnete Geräte
- Organisationen, Dienstprofile und Prioritäten
- Berechtigungen, Sperren und Registrierungsfreigaben
- Import, Export und Änderungsverlauf
- Abhängigkeiten, Datenbankzustand und Audit

### Kritische Aktionen

- Teilnehmer sperren oder freigeben
- Gerätezuordnung ändern
- Dienstprofil und Berechtigungen bearbeiten
- Datensätze importieren oder exportieren

Die Oberfläche zeigt keine kryptografischen Schlüssel an.
