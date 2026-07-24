# Architektur

```text
TBS Edge / Node Gateway ─┐
Subscriber / Group Core ─┤
Mobility / Call Control ─┤
Media / Recorder ────────┤── read-only Lageaggregation ──> Control Room WebUI
SDS / Packet / IP ───────┤
Security / KMF ──────────┤
Transit ─────────────────┘
```

Der Control Room speichert ausschließlich Operatorzustand:

- Incident-Ack und Lösungsstatus,
- Notizen,
- Schichtbuch,
- Command- und Event-Audit,
- zuletzt beobachtete Service-Health-Snapshots.

Fachzustände bleiben in den jeweiligen Diensten. Ein Ausfall des Control Room darf deren Betrieb nicht stoppen. Umgekehrt zeigt der Control Room Ausfälle und Degradierungen der Fachsysteme als Lagebild an.

## Federiertes Lagebild

Der zyklische Poller übernimmt nur Health-, Readiness- und kompakte Status-Snapshots. Daraus erzeugt der Control Room ein kuratiertes Domain-Lagebild, beispielsweise registrierte Teilnehmer aus `subscriber-core`, aktive Rufe aus `call-control`, wartende SDS aus `sds-router` und offene Security-Alarme aus `security-core`.

Diese Werte sind Cache und Anzeige, keine zweite Datenbank. Fachliche Änderungen erfolgen weiterhin über den zuständigen Dienst oder über explizite, typisierte Operator-Kommandos.
