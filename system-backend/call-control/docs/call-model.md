# Modell für logische Calls und TBS-Legs

Ein `LogicalCall` beschreibt einen netzweiten Ruf. Er besitzt einen stabilen UUID-basierten Identifier und eine Operation-ID. Darunter liegen ein oder mehrere `CallLeg`-Datensätze, jeweils genau einer pro beteiligter TBS.

Die zentrale Laufzeit kennt Gruppen- und Individualrufe. Ein Gruppenruf kann mehrere TBS-Legs gleichzeitig besitzen. Ein Individualruf wird zunächst auf der TBS des gerufenen Teilnehmers aufgebaut. Später können zusätzliche Legs durch Mobilität und Restore hinzukommen.

Call Control entscheidet nicht über konkrete TDMA-Ressourcen. Jede TBS führt die lokale CMCE-State-Machine aus und liefert lokale Call-ID, Timeslot, Usage sowie Floor-Zustand zurück.

Beobachtete, nicht zentral gestartete Rufe werden aus TBS-Telemetrie als `managed = false` aufgenommen. Dadurch zeigt die WebUI auch Teilnehmer-initiierte Rufe.
