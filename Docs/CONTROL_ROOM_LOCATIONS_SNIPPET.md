# NetCore Control Room — Locations/Stale-Call Fix

This patch builds on the detail API patch. It adds:

- defensive cleanup of stale `GroupState.active_call_id` references when a group call ends
- defensive `GroupDetail` rendering so groups no longer show an `active_call_id` when `/api/calls` is empty
- parsing of SDS text like `LIP position: 52.398562, 9.644934` into subscriber `last_location`
- `GET /api/locations`
- `GET /api/nodes/{node_id}/locations`
- `last_location` on `/api/subscribers` entries
- `locations` in `/api/nodes/{node_id}` detail output

Test commands:

```bash
curl http://127.0.0.1:9010/api/groups | jq
curl http://127.0.0.1:9010/api/calls | jq
curl http://127.0.0.1:9010/api/locations | jq
curl http://127.0.0.1:9010/api/nodes/tbs-04010001/locations | jq
curl http://127.0.0.1:9010/api/subscribers?online=true | jq
```
