# Phase 4 – TTS Common-SCCH Pinned Delivery Fix

## Beobachtung

Nach dem ersten Common-SCCH-/Frame-18-Fix öffnete das Funkgerät den netzgestarteten
TTS-Gruppenruf weiterhin erst nach einem Re-Register. Der Re-Register-Reannounce-Pfad
funktioniert, die ursprüngliche Rufankündigung erreicht das idle Terminal aber weiterhin
nicht zuverlässig.

## Gefundener Fehler im ersten Frame-18-Fix

Der erste Fix erzeugte zwar das Log

```text
CMCE: network D-SETUP queued for frame-18 common SCCH
```

aber die Nachricht war nicht tatsächlich an Frame 18 gebunden:

1. `CcBsSubentity` übergab das D-SETUP über den normalen CMCE → MLE → LLC → UMAC-Pfad.
2. `build_sapmsg()` ignorierte den übergebenen `dltime`-Wert.
3. MLE setzte `req_handle` wieder auf `0`.
4. UMAC legte die Nachricht in die normale TS1-MCCH-Warteschlange.
5. Der Scheduler konnte sie deshalb im nächsten normalen TS1-Slot verbrauchen, lange bevor
   Frame 18 erreicht wurde.

Das bisherige „queued for frame-18“-Log dokumentierte somit nur die Absicht in CMCE, nicht
den tatsächlichen Luftschnittstellen-Slot.

## Änderungen

### Interner Delivery-Marker

Ein BS-interner Request-Handle-Marker wird durch MLE und LLC bis UMAC erhalten. Er wird nie
über die Luftschnittstelle übertragen.

### Dedizierte Frame-18-Warteschlange

UMAC erkennt den Marker und legt das D-SETUP in eine separate
`frame18_common_scch_queue`. Diese Warteschlange ist für normale MCCH-Slots unsichtbar.

### Echte Slotbindung

Nur ein tatsächlich nutzbarer Primärträger-Slot `Frame 18 / TS1` darf eine Nachricht aus
der dedizierten Warteschlange entnehmen. Ist TS1 durch die zwingende BSCH-/BNCH-Rotation
belegt, bleibt das D-SETUP bis zur nächsten nutzbaren Common-SCCH-Gelegenheit gepuffert.

Normale TS1-MCCH-Nachrichten werden umgekehrt niemals versehentlich in Frame 18 gesendet.

## Neue Nachweis-Logs

Beim Einreihen:

```text
UMAC: routing marked group signalling to dedicated frame-18 common SCCH queue
BsChannelScheduler: queued dedicated frame-18 common SCCH resource
```

Erst bei tatsächlicher Slot-Ausgabe:

```text
BsChannelScheduler: transmitting dedicated frame-18 common SCCH resource carrier=720 ts=.../18/01/...
```

Nur die letzte Meldung beweist, dass die Rufankündigung wirklich im vorgesehenen
Frame-18-/TS1-Slot an PHY übergeben wurde.

## Konfiguration

Der bestehende Audio-Vorlauf bleibt:

```toml
[audio_player]
lead_in_silence_blocks = 40
tail_silence_blocks = 5
group_release_guard_seconds = 7
```

## Betroffene Dateien

- `Docs/PHASE4_TTS_COMMON_SCCH_PINNED_FIX.md`
- `crates/tetra-saps/src/tma/mod.rs`
- `crates/tetra-entities/src/cmce/subentities/cc_bs/pdu.rs`
- `crates/tetra-entities/src/cmce/subentities/cc_bs/timers.rs`
- `crates/tetra-entities/src/mle/mle_bs.rs`
- `crates/tetra-entities/src/umac/umac_bs.rs`
- `crates/tetra-entities/src/umac/subcomp/bs_sched.rs`
