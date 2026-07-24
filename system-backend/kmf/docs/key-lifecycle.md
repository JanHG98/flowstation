# Key-Lifecycle und Crypto Periods

## Zustände

```text
Draft → Staged → Active → Retiring → Retired
                     ↘ Revoked
Draft/Staged/Retired/Revoked → Destroyed
```

`Destroyed` ist final. Dabei wird das Secret aus dem Vault entfernt; Metadaten, Fingerprint und Audit bleiben erhalten.

## Versionierung

Versionen werden pro Kombination aus folgenden Feldern gezählt:

```text
kind + scope + scope_value
```

Beispiele:

```text
GCK / group / 15501 / v1
GCK / group / 15501 / v2
CCK / network / - / v1
```

## Rotation

Eine Rotation:

1. erzeugt neues Zufallsmaterial,
2. erhöht die Version,
3. setzt den bisherigen Key als Vorgänger,
4. setzt den neuen Key als Nachfolger,
5. startet den Nachfolger zunächst in `staged`,
6. kann den aktiven Vorgänger auf `retiring` setzen.

Die Aktivierung ist eine separate Aktion. So kann OTAR vor dem eigentlichen Crypto-Period-Wechsel abgeschlossen werden.

## Überlappung

Überlappende Crypto Periods sind standardmäßig erlaubt, weil ein kontrollierter Übergang zwischen zwei Versionen sonst kaum robust möglich ist. Die Policy kann Überlappung verbieten; dann blockiert die Aktivierung konkurrierender Keys desselben Scopes.

## Zeitgesteuerte Wartung

`POST /api/v1/maintenance/tick`:

- markiert abgelaufene aktive Keys als `retired`,
- beendet abgelaufene OTAR-Aktionen,
- queued nicht quittierte In-Flight-Aktionen erneut oder setzt sie nach Maximalversuchen auf `failed`.
