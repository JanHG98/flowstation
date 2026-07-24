# OTAR-Workflow

## Ablauf

```text
Key vorhanden
    ↓
OTAR Job anlegen
    ↓
Freigabe A
    ↓
Freigabe B
    ↓
Queue
    ↓
pro Node eine Delivery + Action
    ↓
Edge Claim
    ↓
nodegebundener Sealed Envelope
    ↓
ACK / Retry / Failure
```

## Vier-Augen-Prinzip

Standardmäßig werden zwei verschiedene Actor-Namen verlangt. Derselbe Actor kann einen Job nicht zweimal freigeben.

Wichtig: Ohne Login ist das nur eine Workflow-Sperre, keine belastbare Identitätsprüfung. Die produktive Umsetzung benötigt RBAC, authentisierte Operatoren und idealerweise signierte Freigaben.

## Shadow-Modus

Im Shadow-Modus werden Actions als `staged` angelegt. Ein Edge-Claim erhält eine leere Liste. Beim Wechsel auf `authoritative` werden staged Actions zu `pending` hochgestuft.

## Edge-Envelope

Der Claim enthält:

```json
{
  "key_id": "...",
  "key_fingerprint": "...",
  "envelope": {
    "algorithm": "lab_sha256_stream_mac_v1",
    "nonce_hex": "...",
    "ciphertext_hex": "...",
    "mac_hex": "..."
  },
  "envelope_context": "netcore-kmf-otar-edge-v1:action:node:key"
}
```

Das ist kein D-OTAR-PDU. Die Edge muss daraus später die ETSI-konforme Air-Interface-Signalisierung erzeugen.

## Retry

Ein nicht erfolgreich quittierter Claim wird mit Backoff erneut queued. Nach `max_attempts` wird die Action und ihre Delivery als fehlgeschlagen markiert. Jobzustände werden aus allen Deliveries berechnet:

- `completed`,
- `partial_failure`,
- `failed`,
- `in_progress`.
