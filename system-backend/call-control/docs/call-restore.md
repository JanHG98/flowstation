# Call Restore über mehrere TBS

Der zentrale Ablauf besteht aus:

1. aktiven Restore Context auf der Quell-TBS exportieren,
2. Context auf der Ziel-TBS installieren,
3. auf den tatsächlichen Restore des Funkgeräts und die Ziel-Telemetrie warten,
4. den temporären Restore Context auf der Ziel-TBS korreliert entfernen.
4. das neue lokale Ziel-Leg dem bestehenden logischen Call zuordnen.

Der Restore-Vorgang kennt `ExportQueued`, `ExportRequested`, `ImportQueued`, `ImportRequested`, `Ready`, `Completed`, `Cancelled`, `Failed` und `TimedOut`.

Ein Platzhalter-Leg auf der Ziel-TBS verhindert, dass die spätere Restore-Telemetrie versehentlich einen zweiten logischen Call erzeugt. Ein Restore gilt erst als abgeschlossen, wenn ein echtes Ziel-Leg beobachtet wurde. Schlägt danach nur das Aufräumen des temporären Contexts fehl, bleibt der bereits restaurierte Call aktiv und der Fehler erscheint separat in Ereignis und Diagnose.

Media Frames werden in diesem Paket noch nicht zwischen Zellen transportiert. Das übernimmt später der Media Switch. Call Control koordiniert bereits den vollständigen Signalisierungs- und Restore-Kontext.
