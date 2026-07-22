# Mobility Core

## Zweck

Der Mobility Core verwaltet Aufenthaltsort, Erreichbarkeit und Mobilitätszustand aller Teilnehmer.

## Kernaufgaben

- Serving TBS, Serving Cell und Location Area verwalten
- Attach, Detach und Location Updates koordinieren
- Zellwechsel, Migration und Context Transfer steuern
- Home-/Visited-Zustände und Recovery verwalten

## Abgrenzung

Keine Teilnehmerstammdaten, keine Rufsteuerung und keine lokale Funkressourcenverwaltung.

## WebUI zur Verwaltung

Der Mobility Core erhält eine eigene Verwaltungsoberfläche für Registrierungen, Zellen und Kontextwechsel.

### Geplante Ansichten

- aktive Registrierungen und Serving TBS/Cell
- Location Areas und Registration Areas
- Zellwechsel, Migrationen und Context Transfers
- Recovery- und Timeout-Zustände
- Konflikte, Ghost-Kontexte und fehlgeschlagene Übergaben
- Live-Karte beziehungsweise Zellübersicht, sofern Standortdaten vorhanden sind

### Kritische Aktionen

- Teilnehmerkontext kontrolliert freigeben
- erneutes Location Update anfordern
- Context Transfer abbrechen oder wiederholen
- Zelle beziehungsweise TBS in Wartung setzen
