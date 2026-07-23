# SwMI Core 1 – Package F: SDS Router

## Ziel

Package F löst die netzweite SDS-/Statusentscheidung aus der einzelnen TBS heraus, ohne die Air-Interface-Verantwortung der TBS zu verletzen.

## Enthaltene Bausteine

- deploybarer LXC-Dienst `system-backend/sds-router/`
- eigene WebUI und REST-API auf Port 8150
- persistente Store-and-forward-Queue
- Individual-, Gruppen- und Protocol-ID-Routing
- Offline-, Retry-, TTL-, Partial- und Dead-Letter-Verarbeitung
- Duplikaterkennung und Nachrichtentrace
- Anwendungsausgänge mit ACK/NACK
- neuer verlustfreier TBS-Uplink `TelemetryEvent::SdsEdgeIngress`
- neue Downlink-Kommandos `DeliverSds` und `SendStatus`
- explizite Antwort `SdsDeliveryResponse`
- opt-in TBS-Schalter `control_room.central_sds_routing`
- systemd- und LXC-Installationsmaterial
- statischer Checker und CI-Workflow

## Edge/Core-Grenze

Die lokale TBS verarbeitet weiterhin U-/D-SDS-PDUs, MCCH/FACCH, Energy Economy, Wake-up-Fenster sowie lokal zwingende Notruf- und Command-ISSI-Pfade. Der Core speichert und routet die normalisierte Nachricht und bestimmt die zuständigen TBS beziehungsweise Anwendungen.

## Rückwärtskompatibilität

`central_sds_routing` ist standardmäßig `false`. Bestehende TBS arbeiten dadurch nach dem Update zunächst unverändert lokal. Erst nach expliziter Aktivierung liefert die TBS gewöhnliche SDS-/Statusereignisse an den zentralen Router.

## Sicherheitsstand

Wie die bisher implementierten LXC-Dienste läuft Package F im Open-Lab-Modus ohne Tokens, Benutzerkonten und TLS. Das ist ausschließlich für das isolierte Testnetz bestimmt.
