# Floor Control

Floor-Anforderungen werden zentral an alle aktiven Legs eines logischen Rufs verteilt. Die lokale TBS verwendet dafür ihre vorhandenen `U-TX DEMAND`-, Queue- und `U-TX CEASED`-Prozeduren.

Ohne Force entscheidet die lokale CMCE-State-Machine über Grant oder Queue. Mit `force = true` kann der offene Laboroperator einen vorhandenen Sprecher kontrolliert ablösen. Diese Funktion ist standardmäßig konfigurierbar und wegen des fehlenden RBAC deutlich als Laborfunktion markiert.

Call Control führt den zusammengefassten Floor Holder und die aktuell gemeldete Queue. Maßgeblich bleiben die bestätigten Zustände der TBS.
