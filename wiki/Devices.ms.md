<html><body>
<!--StartFragment--><html><head></head><body><h1>Devices</h1><p>Diese Seite beschreibt die Geräteverwaltung im NetCore Directory.</p><p>Geräte sind einzelne TETRA-Teilnehmer im Netz. Jedes Gerät wird über seine ISSI eindeutig identifiziert und kann im Directory mit Namen, Kurzbezeichnung, Typ, Rolle, Besitzer, Farbe, Icon und weiteren Informationen gepflegt werden.</p><p>Ohne Directory sieht die Basisstation nur technische IDs:</p><pre><code class="language-text">2020001
2010002
5102
</code></pre><p>Mit Directory werden daraus lesbare Geräte:</p><pre><code class="language-text">2020001 → HRT Fahrer
2010002 → HRT Reserve
5102    → LIP Testgerät
</code></pre><hr><h2>Aufgabe der Geräteverwaltung</h2><p>Die Geräteverwaltung dient dazu, Funkgeräte eindeutig und lesbar im System abzubilden.</p><p>Sie wird genutzt für:</p><ul><li><p>Dashboard-Anzeige,</p></li><li><p>SDS-Log,</p></li><li><p>Statusanzeige,</p></li><li><p>GPS-/LIP-Karte,</p></li><li><p>Gerätegruppen,</p></li><li><p>Statusgruppen,</p></li><li><p>Fehleranalyse,</p></li><li><p>spätere Fahrzeug-/OPTA-Ansicht,</p></li><li><p>Import und Export von Stammdaten.</p></li></ul><p>Die ISSI bleibt dabei der technische Schlüssel. Alle weiteren Informationen sind Stammdaten, die das System für Anzeige und Logik nutzt.</p><hr><h2>Was ist ein Gerät?</h2><p>Ein Gerät ist ein einzelner Teilnehmer im TETRA-Netz.</p><p>Beispiele:</p><pre><code class="language-text">HRT Fahrer
HRT Beifahrer
MRT Fahrzeug
Gateway-Gerät
Bediengerät
Testgerät
LIP-Testsender
Basisnaher Infrastrukturteilnehmer
</code></pre><p>Jedes Gerät sollte genau eine ISSI besitzen.</p><hr><h2>ISSI</h2><p>Die ISSI ist die eindeutige Teilnehmernummer eines Funkgeräts.</p><p>Beispiel:</p><pre><code class="language-text">2020001
</code></pre><p>Die Basisstation verwendet die ISSI, um das Gerät technisch zu erkennen. Das Directory verwendet dieselbe ISSI, um daraus ein lesbares Gerät zu machen.</p><hr><h2>Empfohlenes ISSI-Schema</h2><p>Ein konsistentes ISSI-Schema erleichtert Betrieb und Fehlersuche.</p><p>Beispielhafte Struktur:</p><pre><code class="language-text">D K EE NNNN
</code></pre><p>Bedeutung:</p>
Teil | Bedeutung
-- | --
D | Domain / Betriebsbereich
K | Klasse
EE | Eigentümer / Betreiber
NNNN | laufende Nummer

<p>Icons sind Hinweise für Dashboard oder Karte. Die genaue Darstellung hängt von der jeweiligen Oberfläche ab.</p><hr><h2>Geräte im Dashboard</h2><p>Das Dashboard nutzt die Gerätedaten aus dem Directory, um ISSIs lesbar darzustellen.</p><p>Beispiel ohne Directory:</p><pre><code class="language-text">ISSI 2020001 ONLINE
</code></pre><p>Beispiel mit Directory:</p><pre><code class="language-text">HRT Fahrer · 2020001 · ONLINE
</code></pre><p>Zusätzlich können angezeigt werden:</p><ul><li><p>Typ,</p></li><li><p>Kurzname,</p></li><li><p>Rolle,</p></li><li><p>Status,</p></li><li><p>Gruppe,</p></li><li><p>letzte Aktivität,</p></li><li><p>GPS-Position,</p></li><li><p>SDS-Aktivität.</p></li></ul><hr><h2>Geräte im SDS-Log</h2><p>Auch SDS-Nachrichten profitieren von Directory-Daten.</p><p>Ohne Directory:</p><pre><code class="language-text">2020001 → 4010001: Status 1
</code></pre><p>Mit Directory:</p><pre><code class="language-text">HRT Fahrer → NetCore-Tetra BS 01: Status 1
</code></pre><p>Dadurch werden Logs deutlich leichter lesbar.</p><hr><h2>Geräte und Statusmeldungen</h2><p>Wenn ein Gerät einen Status sendet, nutzt die Basisstation die ISSI zur Zuordnung.</p><p>Ablauf:</p><pre><code class="language-text">2020001 sendet Status 1
   │
   ▼
Basisstation erkennt ISSI 2020001
   │
   ▼
Directory liefert Gerätenamen
   │
   ▼
Directory liefert Statustext
   │
   ▼
Dashboard zeigt:
HRT Fahrer → Frei auf Funk
</code></pre><p>Wenn das Gerät Mitglied einer Statusgruppe ist, kann der Status auf weitere Geräte übertragen werden.</p><hr><h2>Geräte und Statusgruppen</h2><p>Ein Gerät kann Mitglied einer Gerätegruppe sein.</p><p>Beispiel:</p><pre><code class="language-text">RTW 83-01
├── 2020001 HRT Fahrer
├── 2020002 MRT Fahrzeug
├── 2020003 HRT Beifahrer
└── 2020004 Reservegerät
</code></pre><p>Wenn <code>status_sync</code> für diese Gruppe aktiv ist, wird ein Status eines Mitglieds auf alle Mitglieder angewendet.</p><p>Beispiel:</p><pre><code class="language-text">2020001 sendet Status „Frei auf Funk“
→ 2020001, 2020002, 2020003 und 2020004 übernehmen Status „Frei auf Funk“
</code></pre><p>Dafür muss das Gerät korrekt in der Gerätegruppe eingetragen sein.</p><hr><h2>Geräte und GPS / LIP</h2><p>Wenn ein Gerät Positionsdaten sendet, wird die Position anhand der ISSI zugeordnet.</p><p>Ablauf:</p><pre><code class="language-text">LIP-Position von 2020001
   │
   ▼
Directory:
2020001 = HRT Fahrer
   │
   ▼
Dashboard-Karte:
HRT Fahrer an Position X/Y
</code></pre><p>Ohne Directory kann die Karte nur die ISSI anzeigen.</p><hr><h2>Gerät hinzufügen</h2><p>Ein Gerät kann über die Weboberfläche oder per API hinzugefügt werden.</p><h3>Per Weboberfläche</h3><p>Typischer Ablauf:</p><pre><code class="language-text">1. Directory öffnen
2. Tab „Geräte“ auswählen
3. Neues Gerät hinzufügen
4. ISSI eintragen
5. Name, Kurzname, Typ und Rolle setzen
6. Speichern
</code></pre><p>Danach sollte das Gerät über die API sichtbar sein.</p><hr><h3>Per API</h3><p>Beispiel:</p><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/devices \
  -H 'Content-Type: application/json' \
  -d '{
    "issi": 2020001,
    "name": "HRT Fahrer",
    "short": "Fahrer",
    "type": "HRT",
    "owner": "NetCore",
    "role": "Fahrzeugbesatzung",
    "icon": "radio",
    "color": "green",
    "visible": 1,
    "notes": "Handfunkgerät für Fahrerplatz"
  }'
</code></pre><hr><h2>Gerät bearbeiten</h2><p>Per API:</p><pre><code class="language-bash">curl -X PUT http://127.0.0.1:8095/api/devices/2020001 \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "HRT Fahrer RTW 83-01",
    "short": "Fahrer",
    "type": "HRT",
    "owner": "NetCore",
    "role": "Fahrzeugbesatzung",
    "icon": "radio",
    "color": "green",
    "visible": 1,
    "notes": "aktualisierte Bezeichnung"
  }'
</code></pre><hr><h2>Gerät löschen</h2><p>Per API:</p><pre><code class="language-bash">curl -X DELETE http://127.0.0.1:8095/api/devices/2020001
</code></pre><blockquote><p>Achtung: Vor dem Löschen sollte geprüft werden, ob das Gerät noch in Gerätegruppen, Statusgruppen oder Tests verwendet wird.</p></blockquote><hr><h2>Geräte abrufen</h2><p>Alle Geräte:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices | jq .
</code></pre><p>Ein bestimmtes Gerät:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices/2020001 | jq .
</code></pre><hr><h2>Lookup-kompatible Abfrage</h2><p>Ein Gerät kann auch über den Lookup-Endpunkt abgefragt werden:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/dmr/user/?id=2020001' | jq .
</code></pre><p>Beispielantwort:</p><pre><code class="language-json">{
  "count": 1,
  "results": [
    {
      "id": 2020001,
      "callsign": "Fahrer",
      "fname": "NetCore",
      "surname": "HRT",
      "city": "Fahrzeugbesatzung",
      "state": "HRT Fahrer",
      "country": "NetCore"
    }
  ]
}
</code></pre><p>Dieser Endpunkt ist für einfache ISSI-zu-Name-Auflösung gedacht.</p><hr><h2>Import und Export</h2><p>Geräte werden im Directory-Export im Bereich <code>devices</code> gespeichert.</p><p>Beispiel:</p><pre><code class="language-json">{
  "devices": [
    {
      "issi": 2020001,
      "name": "HRT Fahrer",
      "short": "Fahrer",
      "type": "HRT",
      "owner": "NetCore",
      "role": "Fahrzeugbesatzung",
      "icon": "radio",
      "color": "green",
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
</code></pre><hr><h2>CSV-Übernahme</h2><p>Wenn Geräte aus einer CSV-Datei übernommen werden, sollten die Spalten möglichst eindeutig sein.</p><p>Empfohlene CSV-Spalten:</p><pre><code class="language-csv">issi,name,short,type,owner,role,icon,color,visible,notes
2020001,HRT Fahrer,Fahrer,HRT,NetCore,Fahrzeugbesatzung,radio,green,1,
2020002,MRT RTW 83-01,MRT 83-01,MRT,NetCore,Fahrzeuggerät,vehicle,blue,1,
</code></pre><p>Wichtig:</p><pre><code class="language-text">- ISSI immer als Zahl pflegen
- keine Leerzeichen in ISSI-Feldern
- keine doppelten ISSIs
- sichtbare Geräte mit visible=1 markieren
- alte oder reservierte Geräte mit visible=0 markieren
</code></pre><hr><h2>Doppelte ISSIs vermeiden</h2><p>Jede ISSI darf nur einmal im Directory existieren.</p><p>Problematisch:</p><pre><code class="language-text">2020001 → HRT Fahrer
2020001 → MRT Fahrzeug
</code></pre><p>Das führt zu unklarer Anzeige und fehlerhafter Zuordnung.</p><p>Vor Importen sollte geprüft werden:</p><pre><code class="language-text">- gibt es doppelte ISSIs?
- wurden alte Geräte überschrieben?
- sind Testgeräte klar markiert?
- stimmen Typ und Rolle?
</code></pre><hr><h2>Reservierte ISSIs</h2><p>Es ist sinnvoll, bestimmte ISSI-Bereiche zu reservieren.</p><p>Beispiele:</p><pre><code class="language-text">Basisstationen
Gateways
HRTs
MRTs
Testgeräte
virtuelle Teilnehmer
Service-Teilnehmer
</code></pre><p>Reservierte ISSIs können im Directory entweder bereits als ausgeblendete Geräte angelegt oder separat dokumentiert werden.</p><p>Beispiel:</p><pre><code class="language-json">{
  "issi": 2999999,
  "name": "Reserviert HRT-Testbereich",
  "short": "Reserve",
  "type": "Reserved",
  "owner": "NetCore",
  "role": "ISSI-Reserve",
  "visible": 0
}
</code></pre><hr><h2>Empfohlene Gerätepflege</h2><p>Für jedes reale Gerät sollten mindestens gepflegt sein:</p><pre><code class="language-text">ISSI
Name
Kurzname
Typ
Rolle
Owner
Sichtbarkeit
</code></pre><p>Für Geräte in Fahrzeug-/Statusgruppen zusätzlich:</p><pre><code class="language-text">- eindeutige Rolle im Fahrzeug
- passende Gerätegruppe
- status_sync der Gruppe prüfen
</code></pre><p>Für GPS-Geräte zusätzlich:</p><pre><code class="language-text">- sinnvolle Icon-/Farbauswahl
- Rolle auf Karte eindeutig
- Notiz zu GPS/LIP-Fähigkeit
</code></pre><hr><h2>Typische Gerätesätze</h2><h3>Fahrzeug mit MRT und zwei HRT</h3><pre><code class="language-text">RTW 83-01
├── 2020001 HRT Fahrer
├── 2020002 MRT Fahrzeug
└── 2020003 HRT Beifahrer
</code></pre><h3>Einsatzleitung</h3><pre><code class="language-text">ELW 1
├── 2030001 MRT ELW
├── 2030002 HRT Einsatzleiter
├── 2030003 HRT Funker
└── 2030004 Bediengerät Dashboard
</code></pre><h3>Testumgebung</h3><pre><code class="language-text">Testnetz
├── 2010002 HRT Testgerät
├── 2020001 HRT Status-Test
├── 5102 LIP-Testgerät
└── 4010001 Basisstation
</code></pre><hr><h2>Prüfen, ob ein Gerät korrekt eingetragen ist</h2><p>API-Abfrage:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices/2020001 | jq .
</code></pre><p>Statusgruppen-Abfrage:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Dashboard prüfen:</p><pre><code class="language-text">1. Gerät einschalten
2. Registrierung abwarten
3. Dashboard öffnen
4. Prüfen, ob Name statt nur ISSI angezeigt wird
5. Status oder SDS senden
6. Logs prüfen
</code></pre><hr><h2>Fehleranalyse</h2><h3>Gerät wird nur als ISSI angezeigt</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät fehlt im Directory
- ISSI falsch eingetragen
- Directory nicht erreichbar
- Dashboard nutzt Cache
- visible ist 0
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices/2020001 | jq .
</code></pre><hr><h3>Gerät erscheint nicht im Dashboard</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät ist nicht registriert
- Gerät ist im Ruhezustand
- visible ist 0
- Dashboard-Filter aktiv
- falsche ISSI im Codeplug
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "2020001|Register|Deregister|subscriber"
</code></pre><hr><h3>Statusgruppe erkennt Gerät nicht</h3><p>Prüfen:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät nicht in Gerätegruppe eingetragen
- status_sync deaktiviert
- Mitgliederliste falsch formatiert
- ISSI als Text mit Sonderzeichen importiert
- Gruppe nicht gespeichert
</code></pre><hr><h3>Falscher Name wird angezeigt</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- doppelte oder alte Daten im Directory
- Import hat Gerät überschrieben
- Dashboard-Cache
- falsche ISSI im Funkgerät
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices | jq '.[] | select(.issi == 2020001)'
</code></pre><hr><h3>Gerät sendet Status, aber bekommt keinen HMD-Text</h3><p>Das Gerät selbst kann korrekt eingetragen sein, aber Statusrückmeldung hängt zusätzlich von Statuslogik und Gerätefähigkeit ab.</p><p>Prüfen:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "SDS-STATUS|HomeModeDisplay|2020001"
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Statuscode fehlt im Directory
- Gerät ist nicht registriert
- HMD-Funktion wird vom Gerät nicht unterstützt oder anders angezeigt
- Antwort wurde durch Throttle begrenzt
- falsche Ziel-ISSI
</code></pre><hr><h2>Best Practices</h2><h3>Kurznamen kurz halten</h3><p>Gute Kurznamen:</p><pre><code class="language-text">Fahrer
Beifahrer
MRT 83-01
EL
Funker
Gateway
</code></pre><p>Zu lange Kurznamen erschweren kompakte Dashboard-Ansichten.</p><hr><h3>Typen einheitlich schreiben</h3><p>Nicht mischen:</p><pre><code class="language-text">HRT
Hrt
Handfunkgerät
Handgerät
</code></pre><p>Besser einheitlich:</p><pre><code class="language-text">HRT
MRT
Gateway
Control
Test
</code></pre><hr><h3>Rollen sprechend benennen</h3><p>Gut:</p><pre><code class="language-text">Fahrzeugbesatzung
Fahrzeuggerät
Einsatzleitung
Telefonie-Gateway
LIP-Test
</code></pre><p>Weniger gut:</p><pre><code class="language-text">Gerät 1
Test
Sonstiges
</code></pre><hr><h3>Vor Importen exportieren</h3><p>Vor größeren Änderungen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/export \
  -o netcore-directory-before-device-import-$(date +%F-%H%M).json
</code></pre><hr><h3>Testgeräte klar markieren</h3><p>Testgeräte sollten eindeutig erkennbar sein.</p><p>Beispiel:</p><pre><code class="language-json">{
  "issi": 2010002,
  "name": "HRT Testgerät",
  "short": "Test HRT",
  "type": "Test",
  "owner": "NetCore",
  "role": "Status- und SDS-Test",
  "visible": 1
}
</code></pre><hr><h2>Beispiel: vollständiger Gerätesatz für ein Fahrzeug</h2><pre><code class="language-json">[
  {
    "issi": 2020001,
    "name": "HRT Fahrer RTW 83-01",
    "short": "Fahrer",
    "type": "HRT",
    "owner": "NetCore",
    "role": "Fahrzeugbesatzung",
    "icon": "radio",
    "color": "green",
    "visible": 1,
    "notes": ""
  },
  {
    "issi": 2020002,
    "name": "MRT RTW 83-01",
    "short": "MRT 83-01",
    "type": "MRT",
    "owner": "NetCore",
    "role": "Fahrzeuggerät",
    "icon": "vehicle",
    "color": "blue",
    "visible": 1,
    "notes": ""
  },
  {
    "issi": 2020003,
    "name": "HRT Beifahrer RTW 83-01",
    "short": "Beifahrer",
    "type": "HRT",
    "owner": "NetCore",
    "role": "Fahrzeugbesatzung",
    "icon": "radio",
    "color": "green",
    "visible": 1,
    "notes": ""
  }
]
</code></pre><hr><h2>Zusammenhang mit anderen Wiki-Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[NetCore-Directory]]</p></li><li><p>[[Device-Groups]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[LIP-and-GPS]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>