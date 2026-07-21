# Registrierung und Gruppenbindung

## Registrierung

Ein Endgerät meldet sich mit Location Updating an der Basisstation an. Die Basisstation prüft Zell- und Teilnehmerlogik, bestätigt die Registrierung und hält den Laufzeitstatus des Geräts.

Wichtige Zellschalter:

```toml
[cell_info]
registration = true
deregistration = true
periodic_registration_secs = 3600
```

Bei periodischer Registrierung wird ein nicht rechtzeitig aktualisiertes Gerät erneut angesprochen. Die Basisstation entfernt dabei bewusst nicht sofort alle gespeicherten Gruppenbindungen, weil manche Endgeräte bei einem harten Ablauf problematisch reagieren.

## Gruppenbindung

Die Affiliation beschreibt, welche GSSIs ein Gerät aktuell empfängt. Sie entsteht durch Meldungen des Endgeräts und ist nicht identisch mit einem Directory-Gruppeneintrag.

## Recovery

- Proaktives Replay kann bekannte Geräte nach einem Neustart wieder ansprechen.
- Reaktives Re-Attract fordert unbekannt auftauchende Geräte zur Registrierung auf.
- Eine Allowlist kann den Mechanismus auf ausgewählte ISSIs begrenzen.

## Diagnose

Bei „registriert, aber kein Gruppenruf“ prüfen:

1. wurde die Gruppenliste tatsächlich gemeldet?
2. ist die GSSI in der Laufzeit-Affiliation sichtbar?
3. gab es kurz zuvor einen Reject oder Re-Attract?
4. wurde das Gerät nach Konfigurationsänderung sauber re-registriert?
5. passt der Traffic-Carrier zum Endgerät?
