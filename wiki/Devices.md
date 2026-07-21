# Geräte

Ein Gerät beschreibt ein TETRA-Endgerät bzw. einen Teilnehmer im NetCore Directory.

## Felder

| Feld | Bedeutung |
|---|---|
| `issi` | eindeutige Individual Short Subscriber Identity |
| `name` | vollständige Bezeichnung |
| `short` | kurze Dashboard-Anzeige |
| `type` | Gerätetyp oder Kategorie |
| `owner` | organisatorische Zuordnung |
| `role` | Rolle des Geräts |
| `icon` | Symbol für Dashboard/Karte |
| `color` | Darstellungsfarbe |
| `visible` | Sichtbarkeit in Oberflächen |
| `notes` | freie interne Hinweise |

## Empfehlung

- ISSI ausschließlich numerisch und ohne führende Formatzeichen pflegen.
- Namen eindeutig halten.
- Kurzbezeichnung so wählen, dass sie auch auf kleinen Displays sinnvoll bleibt.
- Organisatorische Daten nicht in den Funknamen quetschen, wenn dafür `owner` und `role` existieren.
- Ausgemusterte Geräte eher unsichtbar setzen oder archivieren, statt IDs neu zu verwenden.

## Beziehung zu Statusgruppen

Ein Gerät kann Mitglied mehrerer Gerätegruppen sein. Ist für eine Gruppe `status_sync` aktiv, kann ein Status dieses Geräts an die weiteren Gruppenmitglieder verteilt und beim erneuten Registrieren wiederholt werden.

## Anzeige in der Basisstation

Ist Directory erreichbar, ersetzt das Dashboard die nackte ISSI durch Name, Kurzname, Farbe und Icon. Die ISSI bleibt die technische Primäridentität.
