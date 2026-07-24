# Service Contract Versioning

- Aktuelle gemeinsame Hauptversion: `netcore.v1`.
- Minor-Erweiterungen müssen abwärtskompatibel und optional sein.
- Major-Änderungen erhalten einen parallelen Adapter oder Endpunkt; kein stiller In-place-Bruch.
- Wiederholbare Commands benötigen `message_id` und `idempotency_key`.
- `correlation_id` verbindet Request, Folgeevents, Audit und Antwort; `causation_id` bezeichnet den direkten Auslöser.
- Service Descriptor und Capabilities entscheiden vor der Nutzung optionaler Funktionen über Kompatibilität.
- Generische Envelopes dürfen kein Rohschlüsselmaterial und keine unredigierten Secrets transportieren.
