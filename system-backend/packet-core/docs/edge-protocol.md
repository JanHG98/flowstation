# Packet Edge Protocol v1

Protokollkennung:

```text
netcore-packet-edge-v1
```

Die HTTP-Referenzschnittstelle ist `POST /api/v1/edge/events`. Sie dient zunächst als klar testbare Grenze, bis dieselben Events direkt in die dauerhafte Edge/Core-Verbindung der TBS übernommen werden.

Unterstützte Ereignisfamilien:

- `hello`, `heartbeat`, `node_lost`
- `subscriber_location`
- `activate_demand`, `context_activated`
- `data_transmit_request`, `end_of_data`, `reconnect`
- `modify`, `deactivate`
- `bearer`, `packet_counters`
- `fragment`

Im Authoritative-Modus antwortet der Core mit versionierten Aktionen wie `activate_accept`, `activate_reject`, `page`, `end_of_data`, `modify`, `deactivate` oder `fragment`.

Zusätzlich nutzt der Core den Node Gateway für bestehende TBS-Kommandos. Die neuen `PacketData*`-Kommandos werden im Stack zur SNDCP-Entity geroutet und mit `PacketDataActionResult` korreliert.
