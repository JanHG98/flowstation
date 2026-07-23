# Fragmentierung, Reassembly und Flow Control

## Reassembly

Ein Datagramm wird über Node, ISSI, NSAPI, Richtung und Datagramm-ID identifiziert. Segmente werden nach Offset sortiert. Überlappende Fragmente werden standardmäßig abgewiesen, weil „last fragment wins“ bei Netzprotokollen eine wunderbare Quelle für sehr unschöne Überraschungen ist.

Grenzen:

- maximale parallele Datagramme,
- maximale Gesamtbytes,
- maximale Fragmente je Datagramm,
- Timeout je Reassembly.

Vollständige N-PDUs landen verlustfrei in der N-PDU-Outbox.

## Downlink

`POST /api/v1/downlink` ordnet das N-PDU einem Kontext zu, prüft Payload- und Queue-Limits und zerlegt es anhand der Kontext-MTU. Ist der Teilnehmer nicht READY, wird zuerst eine Page/Wake-Aktion erzeugt.

## Flow Control

Jeder Kontext besitzt Paket-, Byte- und TTL-Grenzen. Aktionen haben Retry-Intervall, maximale Versuchszahl und eindeutige Sequenzen. Damit ist Rückstau sichtbar und nicht bloß ein schwarzes Loch mit optimistischem Logeintrag.
