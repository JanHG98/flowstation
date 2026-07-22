# Mobility Core

## Zweck

Der Mobility Core verwaltet Aufenthaltsort, Erreichbarkeit und netzweite Mobilitätszustände aller Teilnehmer. Er wird später die autoritative Koordination zwischen mehreren TBS übernehmen, während die Air-Interface-State-Machines lokal in den Basisstationen bleiben.

## Kernaufgaben

- Serving TBS, Serving Cell und Location Area verwalten
- Attach, Detach und Location Updates koordinieren
- Migration und Forward Registration zwischen TBS autorisieren
- Teilnehmerkontexte sicher zwischen Quell- und Ziel-TBS übertragen
- Home-/Visited-Zustände und VASSI-Zuordnungen verwalten
- Zellwechsel-, Restore- und Recovery-Vorgänge korrelieren
- Konflikte und doppelte Serving-Kontexte erkennen
- Audit und Operatorentscheidungen bereitstellen

## TBS-Schnittstelle

Die lokale TBS stellt seit SWMI Mobility 1 Paket C folgende Adapter bereit:

- Export eines `MmClientMobilityContext`
- Import unter einer lokalen ISSI beziehungsweise VASSI
- Bereitstellung eines Kontexts für eine laufende Migration
- Abruf eines durch Forward Registration vorbereiteten Kontexts
- read-only Snapshot laufender Migrationen und Forward Registrations

Das spätere Edge Protocol muss diese fachlichen Daten versioniert transportieren. Rohe interne `SapMsg`-Varianten werden nicht über das Netzwerk übertragen.

## Kontextinhalt

- Home-ISSI und gegebenenfalls lokale VASSI
- Teilnehmerzustand
- Gruppenaffiliationen
- Energy-Saving-Mode und Monitoring Window
- Class of MS
- TEI
- letzter lokaler Handle als Diagnosewert
- Quell-/Ziel-TBS, Location Area und Cell-Identifier
- Restore- und Call-Context-Referenzen

## Abgrenzung

Der Mobility Core übernimmt nicht:

- Air-Interface-PDU-Encoding und -Parsing
- lokale MLE-/MM-Timer
- Endpoint-, Link- oder Timeslot-Steuerung
- Teilnehmerstammdaten
- eigentliche Rufsteuerung
- Sprachtransport

Diese Bereiche verbleiben bei TBS, Subscriber Core, Call Control beziehungsweise Media Switch.

## Fallback

Bei Verlust des Core darf die TBS nur die lokal konfigurierten Fallback-Verfahren ausführen. Neue netzweite Migrationen werden nicht geraten oder mit veralteten Kontexten fortgesetzt. Lokale Calls, SDS und Emergency-Funktionen richten sich nach dem später festgelegten Fallback-Profil.

## WebUI zur Verwaltung

Der Mobility Core erhält eine eigene, unabhängig vom Control Room erreichbare Verwaltungsoberfläche.

### Geplante Ansichten

- aktive Registrierungen mit Serving TBS und Serving Cell
- Home-, Visited- und VASSI-Zuordnungen
- Location Areas und Registration Areas
- laufende Migrationen und Forward Registrations
- Context Transfer mit Quell-/Ziel-TBS und Versionsstand
- Gruppen- und Energy-Economy-Kontext eines Transfers
- Zellwechsel und Call-Restore-Korrelation
- Recovery-, Reject- und Timeout-Zustände
- Konflikte, Ghost-Kontexte und doppelte Serving-Zuordnungen
- TBS- und Zellübersicht, optional später als Karte

### Kritische Aktionen

- Teilnehmerkontext kontrolliert freigeben
- erneutes Location Update anfordern
- Context Transfer abbrechen oder wiederholen
- Konflikt auf eine definierte Serving TBS auflösen
- Migration beziehungsweise Forward Registration ablehnen
- Zelle oder TBS in Wartung setzen

Alle schreibenden Aktionen benötigen RBAC, Bestätigung und Audit-Eintrag. Ein Ausfall der WebUI darf den Mobility-Dienst nicht stoppen.
