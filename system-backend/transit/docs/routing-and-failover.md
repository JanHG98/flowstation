# Routing und Failover

Routen werden nach Dienst, Zielregion und Selector ausgewertet. Die Auswahl sortiert nach:

1. direkter Peer zur Zielregion,
2. höherer Präferenz,
3. niedrigerer Metrik,
4. niedrigerer gemessener Latenz,
5. Peer-Priorität.

Nicht berücksichtigt werden gesperrte oder gewartete Peers, inkompatible Protokollversionen, fehlende Service-Capabilities und Peers, deren Region bereits im Path Vector liegt.

Jede Session enthält Legs pro Zielregion. Der ausgewählte Peer und alle Backup-Peers werden am Leg gespeichert. Fällt der aktive Peer aus, wird ein Backup gewählt und der Envelope erneut in die Queue gelegt. Ohne Backup endet nur das betroffene Leg fehlerhaft; andere Gruppenruf-Legs bleiben erhalten.
