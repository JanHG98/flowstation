# Media-Routing

Der Media Switch fragt regelmäßig `GET /api/v1/calls` beim Call Control ab. Aus aktiven logischen Calls und deren aktiven TBS-Legs entsteht ein Routingindex:

```text
(Node-ID, logischer Timeslot) -> Logical Call ID
```

Ein Uplink-Frame wird niemals an dasselbe Quell-Leg zurückgesendet. Alle anderen aktiven Legs derselben Session erhalten eine eigene Downlink-Kopie mit ihrem jeweiligen logischen Ziel-Timeslot. Offline-, nicht mediafähige oder stummgeschaltete Legs werden übersprungen und in den Diagnosezählern erfasst.

Call Control bleibt Eigentümer von Call-IDs, Legs, Floor und Restore. Der Media Switch erzeugt keine Calls und sendet selbst keine Air-Interface-Signalisierung.
