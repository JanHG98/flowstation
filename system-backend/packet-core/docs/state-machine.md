# Kontext-State-Machine

```text
ACTIVATING
   ├─ Accept ─────────────> STANDBY
   └─ Reject/Timeout ─────> FAILED

STANDBY
   ├─ Data Transmit/Page ─> RESPONSE_WAITING
   ├─ Request accepted ───> READY
   ├─ Modify pause ───────> SUSPENDED
   └─ STANDBY timeout ────> DEACTIVATING/removed

READY
   ├─ activity ───────────> READY (timer refresh)
   ├─ READY timeout ──────> STANDBY + END OF DATA
   ├─ context timer ──────> QUIESCENT
   └─ Modify pause ───────> SUSPENDED

SUSPENDED/QUIESCENT
   ├─ Reconnect/Wake ─────> RESPONSE_WAITING/READY
   └─ STANDBY timeout ────> removed
```

Timer werden als absolute RFC-3339-Deadlines persistiert. Nach einem Neustart läuft die Auswertung weiter; der Dienst fällt also nicht durch einen Neustart in ein fiktives READY zurück.
