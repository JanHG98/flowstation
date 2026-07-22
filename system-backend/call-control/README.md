# Call Control

## Zweck

Call Control ist die zentrale Rufsteuerung für Gruppen-, Einzel- und spätere Broadcast-Rufe.

## Kernaufgaben

- Logische Calls und Call Legs verwalten
- Rufrouting, Floor Control und Sprecherwechsel steuern
- Priorität, Pre-emption und Emergency Calls behandeln
- Late Entry, Call Restore und Release koordinieren

## Abgrenzung

Keine lokale Timeslot-Zuteilung und kein eigentlicher Sprachtransport.

## WebUI zur Verwaltung

Call Control erhält eine eigene Verwaltungsoberfläche für laufende und historische Rufzustände.

### Geplante Ansichten

- aktive Gruppen-, Einzel- und Broadcast-Rufe
- Call Legs je TBS, Gateway und Leitstelle
- Floor Owner, Warteschlange und Sprecherwechsel
- Priorität, Pre-emption, Emergency und Late Entry
- Timer, Restore-Zustände und Rufhistorie
- Richtlinien und Abhängigkeiten

### Kritische Aktionen

- Ruf kontrolliert starten oder beenden
- Floor entziehen beziehungsweise freigeben
- Call Leg hinzufügen, entfernen oder neu aufbauen
- fehlerhaften Rufzustand bereinigen

Notruf- und Pre-emption-Aktionen benötigen erhöhte Rechte und eine explizite Bestätigung.
