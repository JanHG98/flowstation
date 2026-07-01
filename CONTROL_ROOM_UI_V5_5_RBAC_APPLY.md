# NetCore Control Room UI v5.5 – rollenbasierte Module

Dieser Stand baut auf v5.4/v5.3 auf und ändert primär die Windows-UI.

## Ziel

Die UI zeigt Module und Bedienflächen abhängig von der angemeldeten Rolle:

- `viewer`: Lesen, Karte, Teilnehmer, Gruppen, Rufe, SDS, Standorte. Keine Befehle, keine Admin-/Userverwaltung, kein Raw JSON.
- `operator`: Viewer + Befehle/Commands. Keine Admin-/Userverwaltung.
- `admin`: alles inklusive Userverwaltung und Raw JSON.

Der Backend-RBAC bleibt weiterhin die harte Sicherheitsgrenze. Die UI blendet nur zusätzlich unzulässige Module aus, damit Viewer/Operatoren gar nicht erst Admin-Flächen sehen.

## Wichtige Änderungen

- Admin/User wird nur für Admins im Menü angezeigt.
- Commands wird nur für Operator/Admin angezeigt.
- Befehlsbox links wird nur für Operator/Admin angezeigt.
- Raw JSON wird nur für Admins angezeigt.
- „Alle Module als OS-Fenster öffnen“ öffnet nur erlaubte Module.
- Unzulässige OS-Fenster werden automatisch geschlossen.
- `/api/admin/users` wird nur noch als Admin abgefragt, damit keine RBAC-Warnungen durch normale Operator-/Viewer-Logins entstehen.
- Kopfzeile zeigt die erkannte Rolle.

## Erwarteter UI-Header

`Native UI v5.5 · responsive UI · rollenbasierte Module`
