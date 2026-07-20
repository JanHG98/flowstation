# NetCore Control Room UI v5.12.1 – Directory-Namen Fix

Dieses Paket behebt, dass im Status-Tableau und auf der Karte weiterhin ISSIs statt Namen angezeigt wurden.

## Fixes

- `/api/directory` ist jetzt authoritative.
- `operator.toml` überschreibt vorhandene LXC-Directory-Einträge nicht mehr, sondern füllt nur noch fehlende Werte auf.
- Rohes `/api/directory` wird zusätzlich behalten und rekursiv durchsucht.
- Namen werden aus deutlich mehr Feldnamen erkannt:
  - `name`, `display_name`, `displayName`, `label`, `alias`
  - `rufname`, `callsign`, `radioAlias`, `shortName`, `terminalName`
  - `bezeichnung`, `description`, `title`
- Directory darf auch verschachtelt sein oder `{ "directory": ... }` liefern.
- Status-Tableau zeigt oben die Directory-Quelle inklusive erkannter Namensanzahl.
