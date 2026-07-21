# ISSI und GSSI

## ISSI

Die ISSI identifiziert ein einzelnes TETRA-Endgerät oder einen Systemteilnehmer. Technisch liegt sie im Bereich von 0 bis 16.777.215.

Für NetCore wird ein planbares Nummernschema empfohlen. Ein mögliches Schema ist:

```text
D K EE NNNN
```

- `D` – Domäne
- `K` – Kategorie
- `EE` – Einheit/Eigentümer
- `NNNN` – laufende Nummer

Das Schema ist organisatorisch; auf der Luftschnittstelle bleibt es eine numerische ISSI.

## GSSI

Die GSSI identifiziert eine Gesprächsgruppe. Sie ist unabhängig von der ISSI eines Endgeräts. Eine Gruppe kann vielen Geräten zugeordnet sein; ein Gerät kann mehrere Gruppen affiliieren.

## System-ISSIs

Systemteilnehmer wie Basisstation, Audioaussendung, Asterisk-Gateway oder Statussteuerung sollten eigene, dokumentierte ISSIs erhalten. Keine System-ISSI gleichzeitig als normales Handfunkgerät verwenden.

## Regeln

- IDs nicht spontan wiederverwenden.
- Produktions-, Test- und Simulationsbereiche trennen.
- Eigentümer und Verwendungszweck im Directory dokumentieren.
- lokale SSI-Bereiche in `[cell_info].local_ssi_ranges` bewusst setzen.
- Routing über Brew oder PBX getrennt von rein lokalen IDs planen.
