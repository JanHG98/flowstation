<html><body>
<!--StartFragment--><html><head></head><body><h1>Device Groups</h1><p>Diese Seite beschreibt Gerätegruppen im NetCore Directory.</p><p>Gerätegruppen fassen mehrere einzelne ISSIs zu einer logischen Einheit zusammen. Das kann ein Fahrzeug, ein Trupp, eine Funktionseinheit oder eine technische Gruppe sein.</p><p>Beispiel:</p><pre><code class="language-text">RTW 83-01
├── 2020001 HRT Fahrer
├── 2020002 MRT Fahrzeug
├── 2020003 HRT Beifahrer
└── 2020004 Reservegerät
</code></pre><p>Gerätegruppen sind die Grundlage für Status-Sync, Fahrzeuglogik, spätere OPTA-Ansichten und gruppenbezogene Betriebsanzeigen.</p><hr><h2>Aufgabe von Gerätegruppen</h2><p>Gerätegruppen dienen dazu, mehrere Funkgeräte logisch zusammenzufassen.</p><p>Sie werden genutzt für:</p><ul><li><p>Fahrzeugstatus,</p></li><li><p>Status-Sync,</p></li><li><p>Status-Replay,</p></li><li><p>Dashboard-Gruppenansichten,</p></li><li><p>spätere OPTA-/Fahrzeugübersichten,</p></li><li><p>strukturierte Geräteverwaltung,</p></li><li><p>bessere Lage- und Betriebsübersicht,</p></li><li><p>Trennung zwischen Einzelgerät und taktischer Einheit.</p></li></ul><p>Ohne Gerätegruppe wird jedes Gerät einzeln betrachtet.</p><p>Mit Gerätegruppe kann das System erkennen:</p><pre><code class="language-text">Diese Geräte gehören gemeinsam zu RTW 83-01.
</code></pre><hr><h2>Unterschied zu GSSI-Gruppen</h2><p>Gerätegruppen sind nicht dasselbe wie GSSI-Gruppen.</p>
Begriff | Bedeutung
-- | --
GSSI-Gruppe | Funkgruppe / Talkgroup im TETRA-Netz
Gerätegruppe | logische Zusammenfassung mehrerer Geräte
Statusgruppe | Gerätegruppe mit aktivem Status-Sync

<hr><h2>Beispiel</h2><pre><code class="language-json">{
  "group_id": 1,
  "name": "RTW 83-01",
  "short": "83-01",
  "opta": "RK H 83-01",
  "type": "vehicle",
  "owner": "NetCore",
  "members": [
    2020001,
    2020002,
    2020003
  ],
  "status_sync": 1,
  "visible": 1,
  "notes": "Fahrzeuggruppe mit MRT und zwei HRT"
}
</code></pre><hr><h2>Pflichtfelder</h2><p>Minimal sinnvoll:</p><pre><code class="language-json">{
  "name": "RTW 83-01",
  "members": [2020001, 2020002]
}
</code></pre><p>Empfohlen:</p><pre><code class="language-json">{
  "name": "RTW 83-01",
  "short": "83-01",
  "opta": "RK H 83-01",
  "type": "vehicle",
  "owner": "NetCore",
  "members": [2020001, 2020002],
  "status_sync": 1,
  "visible": 1
}
</code></pre><hr><h2>Mitglieder</h2><p>Die Mitgliederliste enthält die ISSIs der Geräte, die zur Gruppe gehören.</p><p>Beispiel:</p><pre><code class="language-json">"members": [2020001, 2020002, 2020003]
</code></pre><p>Wichtig:</p><pre><code class="language-text">- ISSIs als Zahlen pflegen
- keine leeren Einträge
- keine doppelten ISSIs innerhalb derselben Gruppe
- nur real existierende oder bewusst reservierte Geräte eintragen
- Reihenfolge möglichst logisch halten
</code></pre><p>Empfohlene Reihenfolge bei Fahrzeugen:</p><pre><code class="language-text">1. MRT / Fahrzeuggerät
2. HRT Fahrer
3. HRT Beifahrer
4. weitere HRTs
5. Gateways / Sondergeräte
</code></pre><p>Oder alternativ:</p><pre><code class="language-text">1. primäres Statusgerät
2. Fahrzeuggerät
3. weitere Geräte
</code></pre><p>Wichtig ist, dass die Reihenfolge nachvollziehbar bleibt.</p><hr><h2>OPTA / Fahrzeugkennung</h2><p>Das Feld <code>opta</code> kann eine taktische oder fahrzeugähnliche Kennung enthalten.</p><p>Beispiel:</p><pre><code class="language-json">"opta": "RK H 83-01"
</code></pre><p>Die OPTA ist nicht zwingend erforderlich, aber für spätere Dashboard- und Fahrzeugansichten sehr hilfreich.</p><p>Beispiele:</p><pre><code class="language-text">RK H 83-01
ELW 1
TECH 01
NETCORE 01
</code></pre><p>Die konkrete Schreibweise kann frei gewählt werden.</p><hr><h2>Kurzname</h2><p>Der Kurzname wird für kompakte Anzeigen verwendet.</p><p>Beispiele:</p><pre><code class="language-text">83-01
ELW 1
TECH 01
GW TEL
</code></pre><p>Gute Kurznamen sind kurz, eindeutig und dashboardfreundlich.</p><hr><h2>Status-Sync</h2><p>Das Feld <code>status_sync</code> legt fest, ob Statusmeldungen eines Gruppenmitglieds auf alle Mitglieder übertragen werden.</p><pre><code class="language-json">"status_sync": 1
</code></pre><p>bedeutet aktiv.</p><pre><code class="language-json">"status_sync": 0
</code></pre><p>bedeutet deaktiviert.</p><hr><h2>Status-Sync-Ablauf</h2><p>Wenn <code>status_sync</code> aktiv ist:</p><pre><code class="language-text">1. Ein Gerät der Gruppe sendet einen Status
2. Die Basisstation fragt das Directory nach der Gerätegruppe
3. Das Directory liefert alle Status-Sync-Mitglieder
4. Die Basisstation setzt den Status für alle Mitglieder
5. Das Dashboard zeigt den Status bei allen Mitgliedern
6. Registrierte Geräte bekommen eine Display-Rückmeldung
7. Offline-Geräte bekommen den Status beim nächsten Rejoin
</code></pre><p>Beispiel:</p><pre><code class="language-text">2020001 sendet Status 1
Directory-Gruppe: 2020001, 2020002, 2020003
Status 1 = Frei auf Funk

Ergebnis:
2020001 → Frei auf Funk
2020002 → Frei auf Funk
2020003 → Frei auf Funk
</code></pre><hr><h2>Statusgruppe</h2><p>Eine Gerätegruppe mit aktivem <code>status_sync</code> wird praktisch zu einer Statusgruppe.</p><pre><code class="language-text">Gerätegruppe + status_sync = Statusgruppe
</code></pre><p>Das bedeutet:</p><pre><code class="language-text">Der Status gehört nicht mehr nur zum sendenden Gerät,
sondern zur gesamten logischen Einheit.
</code></pre><hr><h2>Live Directory Sync</h2><p>Die Basisstation kann Gruppenmitglieder regelmäßig neu aus dem Directory lesen.</p><p>Dadurch werden Änderungen fast sofort übernommen.</p><p>Beispiel:</p><pre><code class="language-text">1. RTW 83-01 enthält 2020001 und 2020002
2. 2020001 sendet Status „Frei auf Funk“
3. Beide Geräte zeigen „Frei auf Funk“
4. Im Directory werden 2020003 und 2020004 ergänzt
5. Die Basisstation aktualisiert die Gruppe automatisch
6. 2020003 und 2020004 übernehmen ebenfalls „Frei auf Funk“
</code></pre><p>Ein neuer Status muss dafür nicht erneut gesendet werden.</p><hr><h2>Status-Replay</h2><p>Wenn ein Gerät offline ist, wird der letzte bekannte Status gespeichert.</p><p>Ablauf:</p><pre><code class="language-text">1. Gruppe erhält Status „Frei auf Funk“
2. 2020003 ist offline
3. Basisstation merkt sich Status für 2020003
4. 2020003 registriert später neu
5. Basisstation sendet Status erneut an 2020003
</code></pre><p>Dadurch bekommen Geräte nach Neustart, Akkuwechsel oder Funkloch wieder den aktuellen Gruppenstatus.</p><hr><h2>Gruppen im Dashboard</h2><p>Gerätegruppen können später als eigene Einheiten angezeigt werden.</p><p>Beispielansicht:</p><pre><code class="language-text">RTW 83-01 · RK H 83-01
Status: Frei auf Funk

Geräte:
- MRT 83-01      ONLINE
- HRT Fahrer     ONLINE
- HRT Beifahrer  RUHEMODUS
</code></pre><p>Dadurch wird die Anzeige taktischer und weniger gerätezentriert.</p><hr><h2>Einzelgerät vs. Fahrzeugstatus</h2><p>Ohne Gerätegruppe:</p><pre><code class="language-text">2020001 → Frei auf Funk
2020002 → kein Status
2020003 → kein Status
</code></pre><p>Mit Gerätegruppe und Status-Sync:</p><pre><code class="language-text">RTW 83-01 → Frei auf Funk

2020001 → Frei auf Funk
2020002 → Frei auf Funk
2020003 → Frei auf Funk
</code></pre><p>Das ist besonders hilfreich, wenn mehrere Geräte im selben Fahrzeug oder Team genutzt werden.</p><hr><h1>API</h1><h2>Alle Gerätegruppen abrufen</h2><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/device-groups | jq .
</code></pre><p>Beispielantwort:</p><pre><code class="language-json">[
  {
    "group_id": 1,
    "name": "RTW 83-01",
    "short": "83-01",
    "opta": "RK H 83-01",
    "type": "vehicle",
    "owner": "NetCore",
    "members": [2020001, 2020002, 2020003],
    "status_sync": 1,
    "visible": 1,
    "notes": "Fahrzeuggruppe"
  }
]
</code></pre><hr><h2>Einzelne Gerätegruppe abrufen</h2><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/device-groups/1 | jq .
</code></pre><hr><h2>Status-Sync-Mitglieder abfragen</h2><p>Dieser Endpunkt ist besonders wichtig für die Basisstation:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Beispielantwort:</p><pre><code class="language-json">{
  "issi": 2020001,
  "count": 1,
  "groups": [
    {
      "group_id": 1,
      "opta": "RK H 83-01",
      "name": "RTW 83-01",
      "members": [2020001, 2020002, 2020003],
      "status_sync": 1
    }
  ],
  "status_sync_members": [2020001, 2020002, 2020003]
}
</code></pre><p>Die Basisstation verwendet <code>status_sync_members</code>, um den Status auf alle relevanten Geräte anzuwenden.</p><hr><h2>Gerätegruppe hinzufügen</h2><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/device-groups \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "RTW 83-01",
    "short": "83-01",
    "opta": "RK H 83-01",
    "type": "vehicle",
    "owner": "NetCore",
    "members": [2020001, 2020002, 2020003],
    "status_sync": 1,
    "visible": 1,
    "notes": "Fahrzeuggruppe mit MRT und HRTs"
  }'
</code></pre><hr><h2>Gerätegruppe bearbeiten</h2><pre><code class="language-bash">curl -X PUT http://127.0.0.1:8095/api/device-groups/1 \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "RTW 83-01",
    "short": "83-01",
    "opta": "RK H 83-01",
    "type": "vehicle",
    "owner": "NetCore",
    "members": [2020001, 2020002, 2020003, 2020004],
    "status_sync": 1,
    "visible": 1,
    "notes": "Fahrzeuggruppe erweitert"
  }'
</code></pre><hr><h2>Gerätegruppe löschen</h2><pre><code class="language-bash">curl -X DELETE http://127.0.0.1:8095/api/device-groups/1
</code></pre><blockquote><p>Achtung: Vor dem Löschen sollte geprüft werden, ob die Gruppe noch für Status-Sync, Dashboard-Anzeige oder Betriebslogik verwendet wird.</p></blockquote><hr><h1>Import und Export</h1><p>Gerätegruppen werden im Directory-Export im Bereich <code>device_groups</code> gespeichert.</p><p>Beispiel:</p><pre><code class="language-json">{
  "device_groups": [
    {
      "group_id": 1,
      "name": "RTW 83-01",
      "short": "83-01",
      "opta": "RK H 83-01",
      "type": "vehicle",
      "owner": "NetCore",
      "members": [2020001, 2020002, 2020003],
      "status_sync": 1,
      "visible": 1,
      "notes": ""
    }
  ]
}
</code></pre><p>Export:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/export \
  -o netcore-directory-export.json
</code></pre><p>Import:</p><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/import \
  -H 'Content-Type: application/json' \
  --data-binary @netcore-directory-export.json
</code></pre><hr><h2>CSV-Übernahme</h2><p>Für Gerätegruppen aus CSV ist eine klare Mitglieder-Schreibweise wichtig.</p><p>Empfohlene CSV-Struktur:</p><pre><code class="language-csv">name,short,opta,type,owner,members,status_sync,visible,notes
RTW 83-01,83-01,RK H 83-01,vehicle,NetCore,"2020001,2020002,2020003",1,1,Fahrzeuggruppe
ELW 1,ELW 1,ELW 1,command,NetCore,"2030001,2030002,2030003",1,1,Einsatzleitung
</code></pre><p>Wichtig:</p><pre><code class="language-text">- Mitglieder eindeutig trennen
- ISSIs als Zahlen interpretieren
- keine Leerzeichen oder Sonderzeichen in ISSIs
- vor Import Backup erstellen
- nach Import API prüfen
</code></pre><hr><h2>Beispiel: Fahrzeuggruppe</h2><pre><code class="language-json">{
  "name": "RTW 83-01",
  "short": "83-01",
  "opta": "RK H 83-01",
  "type": "vehicle",
  "owner": "NetCore",
  "members": [
    2020001,
    2020002,
    2020003
  ],
  "status_sync": 1,
  "visible": 1,
  "notes": "Reguläres Fahrzeug mit MRT und zwei HRT"
}
</code></pre><hr><h2>Beispiel: Einsatzleitung</h2><pre><code class="language-json">{
  "name": "ELW 1",
  "short": "ELW 1",
  "opta": "ELW 1",
  "type": "command",
  "owner": "NetCore",
  "members": [
    2030001,
    2030002,
    2030003,
    2030004
  ],
  "status_sync": 1,
  "visible": 1,
  "notes": "Einsatzleitung mit MRT, HRT und Bediengerät"
}
</code></pre><hr><h2>Beispiel: Gateway-Gruppe</h2><pre><code class="language-json">{
  "name": "Telefonie Gateway",
  "short": "Tel-GW",
  "opta": "GW TEL",
  "type": "gateway",
  "owner": "NetCore",
  "members": [
    2050001,
    2050002
  ],
  "status_sync": 0,
  "visible": 1,
  "notes": "Gateway-Geräte ohne gemeinsamen Fahrzeugstatus"
}
</code></pre><p>Bei Gateway-Gruppen ist <code>status_sync</code> nicht immer sinnvoll. Das hängt davon ab, ob die Geräte wirklich einen gemeinsamen Status haben sollen.</p><hr><h2>Beispiel: Testgruppe</h2><pre><code class="language-json">{
  "name": "Status Testgruppe",
  "short": "Status-Test",
  "opta": "TEST 01",
  "type": "test",
  "owner": "NetCore",
  "members": [
    2010002,
    2020001,
    5102
  ],
  "status_sync": 1,
  "visible": 1,
  "notes": "Testgruppe für Status-Sync und HMD"
}
</code></pre><hr><h1>Prüfen der Gerätegruppe</h1><h2>Alle Gruppen anzeigen</h2><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/device-groups | jq .
</code></pre><hr><h2>Mitglieder für eine ISSI prüfen</h2><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Erwartet:</p><pre><code class="language-json">{
  "status_sync_members": [
    2020001,
    2020002,
    2020003
  ]
}
</code></pre><hr><h2>Basisstationslogs prüfen</h2><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "status-sync|SDS-STATUS|Directory refresh|2020001|2020002|2020003"
</code></pre><p>Typische Logs:</p><pre><code class="language-text">SDS-STATUS: applying Directory status=1 from ISSI 2020001 to status-sync member(s) [2020001, 2020002, 2020003]
</code></pre><p>Bei Live-Änderungen:</p><pre><code class="language-text">SDS-STATUS: refreshed Directory status-sync from seed ISSI 2020001 status=1 to [2020001, 2020002, 2020003, 2020004]
</code></pre><hr><h1>Fehleranalyse</h1><h2>Gruppe greift nicht</h2><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät ist nicht Mitglied der Gruppe
- status_sync ist deaktiviert
- ISSI falsch geschrieben
- Mitgliederliste falsch formatiert
- Basisstation erreicht das Directory nicht
- falsche Directory-URL in der Config
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><hr><h2>Nur das sendende Gerät bekommt Status</h2><p>Mögliche Ursachen:</p><pre><code class="language-text">- status_sync_members enthält nur die Quell-ISSI
- Gruppe ist nicht aktiv
- Gruppe wurde nicht gespeichert
- Status-Sync ist aus
- Directory-Endpunkt liefert keine weiteren Mitglieder
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq '.status_sync_members'
</code></pre><hr><h2>Neue Mitglieder werden nicht sofort aktualisiert</h2><p>Mögliche Ursachen:</p><pre><code class="language-text">- Live Directory Sync läuft noch nicht
- Cache-Intervall noch nicht abgelaufen
- Gerät hat noch keinen bekannten Statuscache
- neue Mitglieder sind nicht registriert
- Gruppe wurde im Directory nicht gespeichert
</code></pre><p>Prüfen:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "Directory refresh|status-sync|SDS-STATUS"
</code></pre><hr><h2>HMD kommt bei Gruppenmitglied nicht an</h2><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät ist nicht registriert
- Gerät ist im Ruhezustand
- Gerät unterstützt HMD nicht oder zeigt es anders an
- SDS-Zustellung fehlgeschlagen
- Throttle verhindert wiederholte Rückmeldung
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "HomeModeDisplay|2020002|SDS-STATUS"
</code></pre><hr><h2>Gruppe enthält falsche Geräte</h2><p>Mögliche Ursachen:</p><pre><code class="language-text">- falsche ISSI eingetragen
- Importfehler
- altes Gerät nicht entfernt
- Testgerät versehentlich sichtbar
- doppelte Rolle in mehreren Gruppen
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/device-groups | jq .
</code></pre><hr><h2>Ein Gerät ist in mehreren Statusgruppen</h2><p>Das kann absichtlich sein, ist aber meistens problematisch.</p><p>Beispielproblem:</p><pre><code class="language-text">2020001 ist Mitglied in RTW 83-01
2020001 ist zusätzlich Mitglied in ELW 1
</code></pre><p>Wenn beide Gruppen <code>status_sync</code> aktiv haben, kann unklar werden, welche Gruppe den Status bestimmen soll.</p><p>Empfehlung:</p><pre><code class="language-text">Ein Gerät sollte normalerweise nur in einer aktiven Status-Sync-Gruppe sein.
</code></pre><p>Ausnahmen sollten bewusst dokumentiert werden.</p><hr><h1>Best Practices</h1><h2>Eine Gruppe pro taktische Einheit</h2><p>Für Fahrzeuge:</p><pre><code class="language-text">eine Gerätegruppe = ein Fahrzeug
</code></pre><p>Für Teams:</p><pre><code class="language-text">eine Gerätegruppe = ein Trupp / Team
</code></pre><p>Nicht empfohlen:</p><pre><code class="language-text">eine große Gruppe mit allen Geräten
</code></pre><p>Das macht Status-Sync und spätere Fahrzeugansichten unübersichtlich.</p><hr><h2>Status-Sync bewusst aktivieren</h2><p><code>status_sync</code> sollte nur aktiv sein, wenn die Geräte wirklich denselben Status teilen sollen.</p><p>Sinnvoll:</p><pre><code class="language-text">MRT + HRTs eines Fahrzeugs
</code></pre><p>Nicht immer sinnvoll:</p><pre><code class="language-text">alle Geräte einer Funkgruppe
Gateway-Geräte mit eigener Funktion
Testgeräte unterschiedlicher Aufgaben
</code></pre><hr><h2>Geräte vorher sauber anlegen</h2><p>Vor dem Anlegen einer Gerätegruppe sollten alle Mitglieder bereits als Geräte existieren.</p><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices | jq .
</code></pre><hr><h2>Klare Rollen verwenden</h2><p>Gute Rollen:</p><pre><code class="language-text">MRT Fahrzeug
HRT Fahrer
HRT Beifahrer
HRT Reserve
Gateway
Bediengerät
</code></pre><p>Dadurch wird später die Fahrzeugansicht deutlich verständlicher.</p><hr><h2>Gruppe nach Änderung testen</h2><p>Nach jeder Änderung:</p><pre><code class="language-text">1. Gruppe speichern
2. API prüfen
3. Status von einem Mitglied senden
4. Dashboard kontrollieren
5. HMD auf registrierten Geräten prüfen
6. Logs beobachten
</code></pre><p>API:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "status-sync|SDS-STATUS|HomeModeDisplay"
</code></pre><hr><h2>Vor Importen exportieren</h2><p>Vor größeren Änderungen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/export \
  -o netcore-directory-before-device-groups-$(date +%F-%H%M).json
</code></pre><hr><h2>Empfohlene Gruppenstruktur</h2><p>Beispiel:</p><pre><code class="language-text">RTW 83-01
├── MRT RTW 83-01
├── HRT Fahrer
└── HRT Beifahrer

ELW 1
├── MRT ELW
├── HRT Einsatzleiter
├── HRT Funker
└── Bediengerät

Technik 01
├── HRT Technik 1
├── HRT Technik 2
└── Gateway Technik
</code></pre><hr><h2>Zusammenhang mit anderen Wiki-Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[NetCore-Directory]]</p></li><li><p>[[Devices]]</p></li><li><p>[[Groups]]</p></li><li><p>[[Status-Messages]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Live-Directory-Sync]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>