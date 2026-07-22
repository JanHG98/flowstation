# Media Switch

## Zweck

Der Media Switch verteilt Sprach- und Audiodaten zwischen TBS, Leitstellen, Recordern und externen Gateways.

## Kernaufgaben

- TETRA-Sprachströme annehmen und verteilen
- Media Sessions und Call Legs abbilden
- Leitstellen-Audio, Recorder-Taps und SIP/RTP bereitstellen
- Jitterbuffer, Sequenzierung und optionale Transkodierung übernehmen

## Betriebsanforderungen

Niedrige Latenz, vorhersehbare Ressourcen und getrennte Control- und Media-Pfade.

## WebUI zur Verwaltung

Der Media Switch erhält eine eigene Verwaltungsoberfläche für Streams, Media Sessions und Gateways.

### Geplante Ansichten

- aktive Media Sessions und zugehörige Call Legs
- Quelle, Senken, Codec und Transcoding
- Paketverlust, Jitter, Latenz und Bufferzustand
- TBS-, Leitstellen-, Recorder- und SIP/RTP-Verbindungen
- Audiopegel und Stummschaltungen
- Testton- und Loopback-Funktionen

### Kritische Aktionen

- Streamroute neu aufbauen
- Senke stummschalten oder trennen
- Gateway in Wartung setzen
- kontrollierten Audiotest auslösen

Die WebUI ist niemals Teil des zeitkritischen Media-Datenpfads.
