# NetCore Media Switch

Der Media Switch ist der zentrale, eigenständig deploybare LXC-Dienst für den Transport bereits codierter TETRA-Sprachframes zwischen mehreren TBS-Call-Legs.

## Enthalten

- eigener HTTP-Dienst und eigene WebUI auf Port `8130`
- offener Labormodus ohne Login, Tokens, Passwörter oder TLS
- WebSocket-Anbindung an den Node Gateway
- periodischer Abgleich der aktiven logischen Calls und TBS-Legs mit Call Control
- Routing gepackter 35-Byte-TETRA-ACELP-Frames
- feste, begrenzte Jitter-Puffer je Zielstream
- Duplikat-, Unbekannt-Stream- und Überlastschutz
- Stream-Mute, Session-Flush und Testframe-Injection
- Media-Tap-Metadaten für den späteren Recorder
- Injection-Schnittstelle für den späteren Audio-Player und die Media Library
- Prometheus-Metriken, Events, OpenAPI, systemd und LXC-Installationsskripte

## Datenweg

```text
TBS A / UMAC
  -> Control-Room-Node-Worker
  -> Node Gateway /ws/node
  -> Backend WebSocket /ws/backend
  -> Media Switch / Session + Jitter Buffer
  -> Node Gateway
  -> TBS B / UMAC / lokaler DL-Circuit
```

Call Control bleibt Eigentümer der logischen Calls. Der Media Switch liest dessen aktive Legs und erzeugt daraus ausschließlich den Media-Routinggraphen.

## WebUI zur Verwaltung

`http://<LXC-IP>:8130/`

Die Oberfläche zeigt Sessions, TBS-Legs, RX/TX/Drops, Jitter-Puffer, Nodes, Media-Taps und Ereignisse. Kritische Labormodus-Aktionen sind Stream-Mute, Puffer-Flush und Testframe-Injection.

## Sicherheitsstatus

Diese Ausbaustufe unterstützt ausschließlich `security.mode = "open_lab"`. Sie ist nur für ein isoliertes Testnetz vorgesehen. Andere Modi werden beim Start abgewiesen.
