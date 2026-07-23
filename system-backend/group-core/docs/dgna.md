# DGNA

Eine DGNA-Operation wird mit Node-ID, ISSI, GSSI und Attach/Detach angelegt. Der Group Core korreliert Node-Gateway-Request-ID, TBS-Command-ID und die abschließende `GroupDgnaApplied`-Antwort.

Die TBS verwendet bei einem zentral verwalteten Gruppenprofil dessen `class_of_usage`. Ohne zentrale Richtlinie bleibt der bisherige lokale Standardwert erhalten.

`force = true` ist ein bewusster Operator-Override und umgeht die zentrale Gruppen- und Mitgliedschaftsfreigabe sowie die entsprechende lokale Policy-Prüfung. Nicht umgangen werden:

- die Registrierung des Zielteilnehmers auf der TBS
- die technische Gültigkeit der GSSI
- die Fähigkeit der TBS, DGNA zu verarbeiten

Der Open-Lab-Modus besitzt noch kein RBAC; deshalb darf die WebUI nur in einem isolierten Testnetz erreichbar sein.
