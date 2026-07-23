# Zentraler Context Transfer

Ein Transfer läuft in drei bestätigten Schritten:

1. `MobilityExportContext` auf der Quell-TBS,
2. `MobilityImportContext` auf der Ziel-TBS,
3. `MobilityRemoveContext` auf der Quell-TBS.

Die Quelle wird erst entfernt, nachdem die Ziel-TBS den Import bestätigt hat. Scheitert der Export oder Import, bleibt der ursprüngliche Kontext erhalten. Scheitert nur die abschließende Quellbereinigung, wird der Transfer als Fehler markiert und in der WebUI sichtbar.

Übertragen werden derzeit die MM-Daten:

- Home-ISSI,
- Registrierungszustand,
- Gruppen,
- Energy-Saving-Mode und Monitoring Window,
- Class of MS,
- letzter Layer-2-Handle,
- TEI.

Aktive CMCE-Calls werden weiterhin durch die bereits vorhandene Call-Restore-State-Machine behandelt; der Mobility Core transportiert in diesem Paket ausschließlich den MM-Teilnehmerkontext.
