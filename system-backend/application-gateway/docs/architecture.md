# Architektur

## Zuständigkeit

Der Application Gateway ist die Integrations- und Automatisierungsgrenze der SwMI.

```text
Fremdsystem / Webhook
        │
        ▼
Application Gateway
  ├─ Normalisierung
  ├─ Vorlagen
  ├─ Routing
  ├─ Retry / TTL / Dedupe
  ├─ Rate Limit / Circuit Breaker
  └─ Audit / Redaction
        │
        ├────────► SDS Router
        ├────────► Piper TTS
        ├────────► Media Library
        └────────► externe Adapter
```

## Keine zweite fachliche Wahrheit

Der Dienst speichert Zustellungszustände und Connector-Konfiguration, aber keine zweite Teilnehmer-, Gruppen-, Ruf-, Mobility- oder Key-Datenbank. Die fachlichen Eigentümer bleiben Subscriber Core, Group Core, Call Control, Mobility Core, SDS Router, Media Library und KMF.

## Delivery-Modell

Ein Ingress erzeugt ein `EventRecord`. Eine oder mehrere Regeln erzeugen daraus `DeliveryRecord`-Einträge. Jede Delivery besitzt einen unabhängigen Connector, TTL, Versuchszähler und Zielzustand.

```text
received → routed → queued → in_flight → delivered
                              └───────→ retry
                                      └→ dead_letter
```

Im Shadow-Modus endet eine fällige Delivery in `shadowed`, ohne ein Fremdsystem aufzurufen.

## Ausfallisolation

Ein nicht erreichbarer Telegram-, DAPNET- oder GeoAlarm-Connector darf weder SDS noch TETRA-Rufe blockieren. Deshalb laufen alle Fremdzustellungen asynchron über persistente Queues und eigene Circuit Breaker.
