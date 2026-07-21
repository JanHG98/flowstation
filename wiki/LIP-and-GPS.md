# LIP und GPS

Positionsdaten können aus TETRA-LIP-Meldungen übernommen, im Dashboard angezeigt und optional an Directory oder Leitstelle exportiert werden.

## Voraussetzungen

- Endgerät sendet LIP in einem unterstützten Format.
- GPS ist im Endgerät aktiviert und hat einen gültigen Fix.
- ISSI ist bekannt und möglichst im Directory benannt.
- Kartenansicht kann die Positionsquelle erreichen.

## Verarbeitung

Eine Position wird mit ISSI, Zeitstempel und Qualitätsinformationen gespeichert. Alte Positionen dürfen nicht als aktueller Standort missverstanden werden. Die Oberfläche sollte daher Alter und Aktualität sichtbar machen.

## Datenschutz

Positionsdaten sind betriebliche und potenziell personenbezogene Daten. Zugriff, Aufbewahrung und Export müssen zum Einsatzzweck passen. Eine aktivierte Karte ist kein Grund, Positionsdaten unbegrenzt zu speichern.

## Fehlersuche

- SDS-Log auf LIP-Nachricht prüfen.
- Endgeräteprofil und Zieladresse kontrollieren.
- GPS-Fix direkt am Funkgerät prüfen.
- Zeitstempel und Zeitzone vergleichen.
- Directory-/Control-Room-Export getrennt vom lokalen Empfang testen.
