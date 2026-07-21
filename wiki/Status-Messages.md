# Statusmeldungen

Statusmeldungen ordnen numerischen U-STATUS-Codes eine verständliche Darstellung zu.

## Felder

| Feld | Bedeutung |
|---|---|
| `code` | numerischer Statuswert |
| `label` | sichtbare Bezeichnung |
| `severity` | fachliche Gewichtung |
| `description` | ausführliche Erklärung |
| `color` | Darstellungsfarbe |
| `visible` | Sichtbarkeit |

## Verarbeitung

Bei Eingang eines U-STATUS:

- wird der numerische Wert protokolliert,
- Directory liefert – sofern erreichbar – Label und Darstellung,
- das Dashboard aktualisiert den Gerätestatus,
- ein passendes Home-Mode-Display kann beantwortet werden,
- Statusgruppen können synchronisiert werden,
- Notfallstatus kann die lokale Alarmkette auslösen.

## Notfallstatus

Status `0` und systemseitig zugeordnete Notfallwerte werden besonders behandelt. Notfälle bleiben standardmäßig lokal an der Basisstation sichtbar und können optional an Telegram oder die Leitstelle gemeldet werden. Eine externe Weiterleitung sollte bewusst konfiguriert werden.

## Pflegehinweise

- Codes eindeutig halten.
- Label kurz genug für kleine Anzeigen wählen.
- Beschreibung für Bediener verständlich formulieren.
- Farben nicht als einziges Unterscheidungsmerkmal verwenden.
- Unsichtbare Statuswerte weiterhin dokumentieren, falls Endgeräte sie senden können.
