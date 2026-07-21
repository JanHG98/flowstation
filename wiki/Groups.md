# Gruppen

Gruppen beschreiben TETRA-Gruppenadressen mit GSSI. Sie werden für Gruppenrufe, Audioaussendungen, Darstellung und organisatorische Zuordnung verwendet.

## Felder

| Feld | Bedeutung |
|---|---|
| `gssi` | Group Short Subscriber Identity |
| `name` | vollständige Gruppenbezeichnung |
| `short` | kurze Anzeige |
| `type` | fachliche Kategorie |
| `owner` | organisatorischer Eigentümer |
| `color` | Darstellungsfarbe |
| `visible` | Sichtbarkeit |
| `notes` | interne Hinweise |

## Funkseitige Bedeutung

Ein Directory-Eintrag allein affiliiert kein Funkgerät. Die Gruppenbindung entsteht durch Endgeräte-Signalisierung und lokale Ruflogik. Directory ergänzt Namen und Struktur.

## Aufzeichnung

Bei `recording.mode = "selected_groups"` müssen die aufzuzeichnenden GSSIs zusätzlich in der lokalen Basisstationskonfiguration stehen. Directory-Sichtbarkeit ersetzt diese Liste nicht.

## Benennungsempfehlung

- volle Bezeichnung für Bedienoberflächen
- kurze, eindeutige Abkürzung für kompakte Ansichten
- GSSI nicht aus dem Namen ableiten, sondern als eigenes Primärfeld pflegen
