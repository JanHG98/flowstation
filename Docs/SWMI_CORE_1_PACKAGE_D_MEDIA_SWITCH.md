# SWMI Core 1 – Paket D: Media Switch

## Ziel

Dieses Paket ergänzt den ersten netzweiten Media-Datenpfad zwischen mehreren TBS. Call Control bleibt Eigentümer der logischen Calls und lokalen Legs; der Media Switch übernimmt ausschließlich die Verteilung bereits codierter TETRA-Sprachframes.

## Architektur

```text
TBS A / UMAC UL
  -> lokaler bounded media channel
  -> Control-Room-Node-Worker
  -> Node Gateway
  -> Media Switch
       - Call-Control-Abgleich
       - Session-/Leg-Routing
       - Duplikatprüfung
       - begrenzter Jitter-Puffer
       - Media-Tap
  -> Node Gateway
  -> TBS B / lokaler UMAC-DL-Circuit
```

Die RF-/TDMA-Threads führen keine Netzwerkoperationen aus. Uplink und Downlink verwenden begrenzte, nicht blockierende In-Process-Queues.

## Sprachformat

Die erste Ausbaustufe transportiert TETRA Speech Service 0 als bereits codierten Frame. Die vorhandenen 274 Nutzbits werden in 35 Bytes gepackt. Der Media Switch transkodiert nicht und verändert die Nutzdaten nicht. Sprachframes sind am Node Gateway ein explizit abonnierbarer Hochraten-Topic; andere Backend-Dienste erhalten diesen Traffic nicht.

## Routing

Call Control wird standardmäßig alle zwei Sekunden über `/api/v1/calls` abgefragt. Für jeden aktiven logischen Call werden die aktiven TBS-Legs nach Node und logischem Timeslot indexiert. Ein Uplink-Frame wird an alle anderen aktiven Legs derselben Session verteilt. Das Quell-Leg erhält ohne explizite Loopback-Konfiguration keine Kopie.

## Robustheit

- maximaler Puffer pro Zielstream
- globale Pending-Frame-Grenze
- Abwurf rückwärts laufender oder duplizierter Sequenzen
- Abwurf unbekannter Quellstreams
- keine Zustellung an offline oder nicht mediafähige Nodes
- keine Blockade des TDMA-Routers bei langsamer Backend-Verbindung
- keine persistenten Gateway-Events pro Sprachframe
- feste Obergrenze pro Gateway-Tick

## WebUI

Der Dienst besitzt eine eigene WebUI auf Port 8130. Sie zeigt:

- Media-Sessions und Call-Control-Zustand
- TBS-Legs und logische Timeslots
- RX-, TX- und Drop-Zähler
- Jitter-Puffer pro Ziel
- Node-Gateway- und Call-Control-Verbindung
- Media-Tap-Metadaten
- Ereignisse

Die offenen Labormodus-Aktionen sind Stream-Mute, Session-Flush und Testframe-Injection.

## Recorder und Audio-Player

Der Media-Tap liefert bereits Session-/Quell-/Sequenzmetadaten. Die Injection-API akzeptiert exakt 35 gepackte Bytes. Damit sind die stabilen Anschlussstellen für die kommenden LXC-Dienste Recorder und Media Library/Audio Player vorhanden, ohne diese Dienste in den zeitkritischen Routingpfad einzubauen.

## Sicherheitsmodus

Dieses Paket implementiert ausschließlich `open_lab`:

- keine Tokens
- keine Passwörter
- kein Login
- kein TLS
- keine Client-Zertifikate
- kein RBAC

Andere Security-Modi werden beim Start abgewiesen.
