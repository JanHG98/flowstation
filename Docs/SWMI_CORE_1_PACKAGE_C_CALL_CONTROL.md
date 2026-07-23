# SWMI Core 1 – Paket C: Call Control

## Ziel

Dieses Paket ergänzt `netcore-call-control` als fünften eigenständig deploybaren LXC-Dienst. Der Dienst verwaltet netzweite logische Calls, während die zeitkritische Air-Interface-Ausführung weiterhin lokal in CMCE auf jeder TBS bleibt.

## Enthalten

- zentrale logische Gruppen- und Individualrufe
- TBS-Auswahl aus Gruppenaffiliationen und Teilnehmerregistrierungen
- lokale Gruppen- und Individualruf-Legs über typisierte Control Commands
- Korrelation über Request-ID, Command-ID, Handle und Operation-ID
- Priorität und Notrufkennzeichnung
- Floor-Anforderung, Queueing, Release und Labor-Force-Handover
- Erkennung Teilnehmer-initiierter Rufe aus TBS-Telemetrie
- mehrzellige Call-Restore-Koordination
- Export und Import vollständiger CMCE-Restore-Contexts
- persistente Call-, Leg- und Restore-Historie
- Timeout-, Offline-, Teilstart- und Neustartbehandlung
- eigene WebUI, REST-API, Metriken und OpenAPI
- systemd-Unit und vollständige Installationsskripte

## Open Lab

Der Dienst läuft ausschließlich mit `security.mode = "open_lab"`. Token-, Passwort-, Login- und TLS-Felder existieren in dieser Phase nicht. Force-Floor und alle Managementaktionen sind für jeden erreichbaren Client verfügbar und gehören deshalb ausschließlich in das isolierte Testnetz.

## TBS-Verhalten

Call Control sendet keine Air-Interface-PDUs selbst. Neue Control Commands werden im Node Gateway zur zuständigen TBS transportiert und dort an CMCE zugestellt. CMCE verwendet die bereits vorhandenen Call-, Floor-, Queue-, Release- und Restore-State-Machines. Die TBS bestätigt lokale Call-ID, Timeslot, Usage und Floor-Zustand.

Die TBS meldet zusätzlich die Capabilities `call_control` und `call_restore_context`. Nicht kompatible Nodes werden nicht als Ziele ausgewählt.

## Call Restore

Der zentrale Restore-Prozess exportiert den aktiven Context auf der Quell-TBS, installiert ihn auf der Ziel-TBS und wartet anschließend auf das echte Restore-Leg aus der Zieltelemetrie. Erst dann wird der Vorgang als abgeschlossen markiert. Ein Ziel-Platzhalter verhindert die Anlage eines zweiten logischen Calls. Nach erfolgreicher Wiederherstellung wird der nur für den Übergang benötigte Restore Context auf der Ziel-TBS korreliert entfernt; ein Cleanup-Fehler beendet den bereits laufenden Ruf nicht rückwirkend.

## Bewusste Grenzen

- noch kein netzweiter Sprachframe-Transport; dieser folgt im Media Switch
- keine zentrale Audio-Mischung oder Jitter-Pufferung
- keine produktive Authentisierung oder RBAC
- JSON-Datei statt externer Datenbank für die Laborphase
- Operator-gestartete Gruppenlegs verwenden zunächst den vorhandenen lokalen Network-Call-Origin
- Operator-gestartete Individuallegs verwenden den bestehenden Network-Circuit-Setup-Pfad
