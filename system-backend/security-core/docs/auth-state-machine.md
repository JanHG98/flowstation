# Authentisierungs-Zustandsmaschine

```text
Start
  ├─ Policy ohne Auth ───────────────> authenticated
  └─ Auth erforderlich
       └─ challenge_pending
            └─ Edge claim/dispatch ─> awaiting_response
                 ├─ gültig ─────────> authenticated
                 │                     └─ Class 3: DCK pending_install → active
                 ├─ ungültig ───────> retry oder rejected
                 ├─ Timeout ────────> expired
                 └─ Operator ───────> revoked
```

Jeder Kontext besitzt TTL, Versuchszähler, Knotenbindung und Fingerprints. Roh-Challenge, erwartete Antwort und DCK liegen ausschließlich im Arbeitsspeicher. Nach einem Neustart werden offene Authentisierungen abgebrochen und DCK-Kontexte widerrufen, statt mit unvollständigem Geheimzustand weiterzulaufen.
