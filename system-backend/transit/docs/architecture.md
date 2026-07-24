# Architektur

```text
Region A Core                           Region B Core
┌──────────────────────┐               ┌──────────────────────┐
│ Mobility / Calls/SDS │               │ Mobility / Calls/SDS │
└──────────┬───────────┘               └──────────┬───────────┘
           │ local submit                           │ local delivery
┌──────────▼───────────┐   netcore-transit-v1   ┌──▼──────────────────┐
│ Transit A            ├────────────────────────► Transit B           │
│ routes, sessions,    │◄────────────────────────┤ routes, sessions,   │
│ dedupe, failover     │   heartbeat/envelope   │ dedupe, failover    │
└──────────────────────┘                         └─────────────────────┘
```

Transit trennt die lokale Core-API von der Peer-API. Lokale Dienste geben semantische Ereignisse ab und holen für die eigene Region bestimmte Zustellungen ab. Peer-Transport bleibt ein eigener Hop mit Path Vector, Dedupe und TTL.
