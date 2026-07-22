# ETSI-Quellenregister für die SwMI-Roadmap

## Zweck

Dieses Register hält fest, welche ETSI-Dokumente für die einzelnen NetCore-Tetra-Ausbaustufen als Referenz dienen. Es ist keine Kopie der Normen und ersetzt nicht die Prüfung der jeweils gültigen Fassung.

## Primäre Basis für SWMI Foundation 1

### EN 300 392-2 – Air Interface

Verwendung für:

- MAC, LLC, MLE, MM und CMCE,
- SAP-Primitiven,
- PDU-Codierung,
- Zellwahl und Zellwechsel,
- Timer, Zustände und Fehlerpfade,
- TLPD- und TLMC-nahe Verfahren.

Der vorhandene Quellcode verweist überwiegend auf Klauseln aus EN 300 392-2 V3.8.1. Diese Fassung bleibt daher zunächst die feste Referenz für bestehende Klauselnummern. Neuere Fassungen werden später als Delta bewertet, statt während Paket B verschiedene Stände zu vermischen.

### ETS 300 392-14 – PICS-Proforma

Verwendung für:

- Struktur der Konformitätsmatrix,
- Aufteilung nach Fähigkeiten, Verfahren, PDUs, Elementen, Timern und Konstanten,
- Erfassung von CMCE, MM, MLE, LLC und MAC.

Hinweis: Das Dokument ist historisch und auf die MS-Seite ausgerichtet. Für NetCore-Tetra dient es nur als methodisches Raster, nicht als alleinige aktuelle Normquelle.

### EN 300 394-1 – Conformance Testing, Radio

Verwendung für spätere:

- RF- und Funkmessungen,
- definierte Testkonfigurationen,
- nachvollziehbare On-Air-Abnahme.

Die statische Inventur in Paket A behauptet ausdrücklich keine Konformität nach EN 300 394-1.

## Weitere bereits vorgesehene Quellen

| Bereich | ETSI-Dokument | Geplante Roadmap-Phase |
| --- | --- | --- |
| allgemeines Netzdesign | EN 300 392-1 | Core-Architektur und Referenzpunkte |
| Security | EN 300 392-7 | Phase 12 |
| Supplementary Services allgemein | EN 300 392-9 | Phase 13 |
| Supplementary Services Stage 1/2/3 | EN 300 392-10/-11/-12 | Phase 13 |
| ISI Mobility | EN 300 392-3-15 | Phase 15 |
| ISI Group Call | EN 300 392-3-3/-13 | Phase 15 |
| ISI SDS | EN 300 392-3-4 | Phase 15 |
| ISI Speech Format | EN 300 392-3-8 | Phase 15 |
| TETRA Codec | EN 300 395-2 | Media Switch und Codec-Abnahme |
| PEI | EN 300 392-5 | spätere Terminal-/Gateway-Integration |
| TSIM/UICC | TS/ES 100/200 812 und EN 300 812 | Security/KMF, sofern benötigt |

## Versionsregel

1. Jede Implementierungsphase nennt eine feste Referenzfassung.
2. Klauselnummern im Code müssen diese Fassung erkennen lassen.
3. Änderungen durch neuere Normfassungen werden in einer Delta-Datei dokumentiert.
4. Entwürfe werden als Entwürfe gekennzeichnet und nicht stillschweigend als veröffentlichte Norm behandelt.
5. On-Air-Verhalten realer Geräte wird getrennt von der normativen Sollbeschreibung dokumentiert.

## Ablageregel

Die ETSI-PDFs selbst werden nicht in automatischen ZIP-Lieferungen dupliziert. Im Repo liegen nur:

- Quellenregister,
- Implementierungsmatrix,
- Klauselverweise,
- eigene Testergebnisse,
- keine veränderten Kopien der Normtexte.
