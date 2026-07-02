# NetCore Control Room – Detail API

Dieser Patch erweitert den Control-Room-Core um leitstellentaugliche Detail-Endpunkte. `/api/overview` bleibt der schlanke Dashboard-Einstieg, die neuen Endpunkte liefern gezielt Teilnehmer, Gruppen, aktive Rufe, SDS und Notrufe.

## Globale Endpunkte

```bash
curl http://127.0.0.1:9010/api/subscribers | jq
curl 'http://127.0.0.1:9010/api/subscribers?online=true' | jq
curl http://127.0.0.1:9010/api/groups | jq
curl http://127.0.0.1:9010/api/calls | jq
curl 'http://127.0.0.1:9010/api/sds?limit=100' | jq
curl 'http://127.0.0.1:9010/api/emergencies?active=true' | jq
```

## Node-spezifische Endpunkte

```bash
curl http://127.0.0.1:9010/api/nodes/tbs-04010001 | jq
curl 'http://127.0.0.1:9010/api/nodes/tbs-04010001/subscribers?online=true' | jq
curl http://127.0.0.1:9010/api/nodes/tbs-04010001/groups | jq
curl http://127.0.0.1:9010/api/nodes/tbs-04010001/calls | jq
curl 'http://127.0.0.1:9010/api/nodes/tbs-04010001/sds?limit=50' | jq
curl 'http://127.0.0.1:9010/api/nodes/tbs-04010001/emergencies?active=true' | jq
```

## Zweck

- `/api/subscribers`: Teilnehmerliste mit ISSI, Online-Status, Gruppen, RSSI, Emergency-Flag, letzter Aktivität und aktiven Call-Keys.
- `/api/groups`: Gruppenliste mit GSSI, Mitgliedern, Online-Mitgliedern und aktivem Call.
- `/api/calls`: aktive Gruppen- und Einzelrufe mit Carrier, Timeslot, Priority, Sprecher/Teilnehmern und Zeitstempeln.
- `/api/sds`: jüngste SDS-Nachrichten pro Node oder global.
- `/api/emergencies`: aktive oder historische Notrufe.
- `/api/nodes/{node_id}`: kompakte Detailansicht für genau eine Basisstation.
