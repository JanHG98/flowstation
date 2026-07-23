# Architektur und Datenfluss

## Uplink

```text
MS
  → U-SDS-DATA / U-STATUS
  → lokale TBS-CMCE/SDS-Edge
  → TelemetryEvent::SdsEdgeIngress
  → Control-Room-Verbindung
  → Node Gateway Backend-WebSocket
  → SDS Router
  → Routing, Persistenz, TTL, Duplikaterkennung
```

`SdsEdgeIngress` enthält eine stabile Message-ID, Ingress-Art, Source/Destination, Group-Flag, SDS-Typ, Protocol-ID, exakte Bitlänge, Payload und Priorität. Der Router verändert die Nutzdaten nicht.

## Downlink

```text
SDS Router
  → ControlCommand::DeliverSds oder SendStatus
  → Node Gateway
  → zuständige TBS
  → CMCE/SDS-Edge rekonstruiert D-SDS-DATA oder D-STATUS
  → Air Interface
```

Die TBS beantwortet die Annahme mit `ControlResponse::SdsDeliveryResponse`. Das ist eine **Edge-Annahme**, noch kein garantierter Terminal-Zustellbericht. Ein später empfangener SDS-TL-Report wird separat mit der Nachricht korreliert.

## Routingreihenfolge

1. explizit erzwungene TBS aus dem API-Auftrag,
2. passende Individual-/Gruppen-Node-Regeln,
3. für Individual-SDS die zuletzt bekannte Teilnehmer-TBS,
4. für Gruppen-SDS alle TBS mit beobachteter Gruppenaffiliation,
5. passende Protocol-ID-Anwendungsregeln.

Ohne verfügbares Ziel bleibt die Nachricht im Zustand `offline` gespeichert und wird bei neuer Präsenz erneut geplant.
