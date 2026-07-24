# Alarmmodell

Eine Regel prüft eine konkrete Metrik mit Comparator und Schwellwert. Optionale Filter begrenzen sie auf Service oder Target. `for_secs` erzeugt zunächst `pending` und erst nach Ablauf `firing`.

Alarmzustände:

```text
inactive -> pending -> firing -> resolved
```

Acknowledge ändert nicht die technische Ursache; es markiert nur die betriebliche Kenntnisnahme. Silences werden nach Regel, Service, Target, Severity und Labels gematcht und besitzen immer ein Ablaufdatum. Eine manuelle Resolve-Aktion ist möglich, die Regel kann bei weiter bestehender Ursache beim nächsten Lauf erneut auslösen.
