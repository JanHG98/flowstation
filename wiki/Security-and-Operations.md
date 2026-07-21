# Sicherheit und Betrieb

## Netzwerkgrenzen

Dashboard, Directory, Piper und Control Room sind Verwaltungsdienste. Sie gehören in ein getrenntes Managementnetz oder hinter einen abgesicherten Reverse Proxy. Ein Portforwarding aus dem Internet ist keine sinnvolle Standardlösung.

## Geheimnisse

Nicht in Git, Wiki, Screenshots oder Support-Logs veröffentlichen:

- Passwörter
- Tokens
- Bot-Schlüssel
- SIP-/Brew-Zugangsdaten
- interne Teilnehmerlisten
- private IP-Pläne, wenn sie nicht für die Fehlersuche notwendig sind

## Minimalrechte

- eigener Systembenutzer pro Dienst, soweit praktikabel
- Konfiguration `0600`
- Schreibrechte nur auf benötigte Verzeichnisse
- keine globale Schreibbarkeit von NFS- oder Medienpfaden
- Dashboard-Systemaktionen gezielt absichern

## Funkbetrieb

- nur zulässige Frequenzen und Leistungen verwenden
- Lasttests kontrolliert durchführen
- Notfall- und Systemstatus nicht mit realen Einsatznetzen vermischen
- Simulationsteilnehmer und produktive ISSIs trennen
- bei Änderungen an Antennen, Filtern oder Verstärkern Spektrum erneut prüfen

## Protokollierung

Logs können ISSIs, Gruppen, Texte und Standorte enthalten. Aufbewahrung und Weitergabe müssen zum Testzweck passen. Bei Support-Auszügen sensible Werte schwärzen, aber Zeitstempel und technische Fehlermeldungen erhalten.

## Notfalllogik

Notfallereignisse bleiben standardmäßig lokal. Externe Weiterleitung an Telegram, Brew oder Leitstelle nur aktivieren, wenn Empfänger, Eskalation und Rücknahme eindeutig definiert sind.
