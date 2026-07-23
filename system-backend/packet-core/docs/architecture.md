# Architektur

```text
TBS SNDCP/PDCH Edge
        │ Telemetrie + Control
        ▼
Node Gateway :8080
        │ Backend-WebSocket
        ▼
Packet Core :8160
        ├── Context State Machine
        ├── Address/Anchor Registry
        ├── PDCH/Bearer View
        ├── Fragment Reassembly
        ├── Flow Control / Action Queue
        └── WebUI + REST + Metrics

später:
Packet Core ── N-PDU/Context API ──> IP Gateway
```

Der Packet Core ist keine zweite MAC- oder LLC-Implementierung. Zeitkritische Funkentscheidungen bleiben an der TBS. Zentralisiert werden nur Zustände, Policy und TBS-übergreifende Zuordnung.

## Betriebsmodi

- `shadow`: TBS-Telemetrie ist führend; die zentrale State Machine wird zum Vergleich gepflegt.
- `authoritative`: das Edge-Protokoll erzeugt Antworten und Aktionen aus dem zentralen Zustand.

Ein Wechsel in `authoritative` sollte erst nach stabilen Shadow-Vergleichen erfolgen.
