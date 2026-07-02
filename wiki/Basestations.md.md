<html><body>
<!--StartFragment--><html><head></head><body><h1>Basestations</h1><p>Diese Seite beschreibt die Verwaltung von Basisstationen im NetCore Directory.</p><p>Basisstationen sind die Infrastrukturteilnehmer des Netzes. Sie stellen die lokale TETRA-Zelle bereit, nehmen Registrierungen entgegen, verarbeiten SDS, Statusmeldungen, Gruppenanbindungen, Rufe und liefern Telemetriedaten an Dashboard und weitere Dienste.</p><p>Im Directory werden Basisstationen mit ihrer ISSI, ihrem Namen, Standort, Netzbezug und weiteren Stammdaten gepflegt.</p><p>Beispiel:</p><pre><code class="language-text">4010001 → NetCore-Tetra BS 01
</code></pre><hr><h2>Aufgabe der Basisstationsverwaltung</h2><p>Die Basisstationsverwaltung dient dazu, Infrastrukturkomponenten eindeutig und lesbar im System abzubilden.</p><p>Sie wird genutzt für:</p><ul><li><p>Dashboard-Anzeige,</p></li><li><p>SDS-Log,</p></li><li><p>Statusrückmeldungen,</p></li><li><p>Netzübersicht,</p></li><li><p>spätere Multi-BS-Ansichten,</p></li><li><p>Standortdokumentation,</p></li><li><p>Fehlersuche,</p></li><li><p>Import und Export von Stammdaten.</p></li></ul><p>Technischer Schlüssel ist die ISSI der Basisstation.</p><hr><h2>Was ist eine Basisstation?</h2><p>Eine Basisstation ist die aktive Funkzelle im TETRA-System.</p><p>Sie übernimmt:</p><ul><li><p>Aussendung der Zelle,</p></li><li><p>Annahme von Registrierungen,</p></li><li><p>Verwaltung aktiver Teilnehmer,</p></li><li><p>Verarbeitung von Gruppenanbindungen,</p></li><li><p>Gruppenrufe,</p></li><li><p>Einzelrufe,</p></li><li><p>SDS,</p></li><li><p>Statuslogik,</p></li><li><p>LIP-/GPS-Verarbeitung,</p></li><li><p>Dashboard-Telemetrie,</p></li><li><p>optionale Gateway-Anbindungen.</p></li></ul><p>Für das Directory ist die Basisstation ein eigener Eintrag, ähnlich wie ein Gerät, aber mit Infrastrukturrolle.</p><hr><h2>ISSI der Basisstation</h2><p>Die Basisstation besitzt eine eigene ISSI.</p><p>Beispiel:</p><pre><code class="language-text">4010001
</code></pre><p>Diese ISSI kann verwendet werden für:</p><ul><li><p>SDS-Zieladresse,</p></li><li><p>Statusrückmeldungen,</p></li><li><p>Dashboard-Quelle,</p></li><li><p>Steuerfunktionen,</p></li><li><p>interne Systemkommunikation,</p></li><li><p>Anzeige im Directory.</p></li></ul><p>Wichtig ist, dass diese ISSI eindeutig ist und nicht gleichzeitig als normales Funkgerät verwendet wird.</p><hr><h2>Empfohlenes ISSI-Schema</h2><p>Ein mögliches Schema:</p><pre><code class="language-text">D K EE NNNN
</code></pre><p>Bedeutung:</p>
Teil | Bedeutung
-- | --
D | Domain / Betriebsbereich
K | Klasse
EE | Eigentümer / Betreiber
NNNN | laufende Nummer

<p>Die genaue Darstellung hängt vom Dashboard ab.</p><hr><h2>Basisstation hinzufügen</h2><p>Eine Basisstation kann über die Weboberfläche oder per API hinzugefügt werden.</p><h3>Per Weboberfläche</h3><p>Typischer Ablauf:</p><pre><code class="language-text">1. Directory öffnen
2. Tab „Basisstationen“ auswählen
3. Neue Basisstation hinzufügen
4. ISSI eintragen
5. Name, Kurzname und Standort setzen
6. MCC/MNC dokumentieren
7. Speichern
</code></pre><hr><h3>Per API</h3><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/basestations \
  -H 'Content-Type: application/json' \
  -d '{
    "issi": 4010001,
    "name": "NetCore-Tetra BS 01",
    "short": "BS 01",
    "location": "Hannover",
    "mcc": "901",
    "mnc": "1510",
    "color": "blue",
    "visible": 1,
    "notes": "Haupt-Basisstation"
  }'
</code></pre><hr><h2>Basisstation bearbeiten</h2><pre><code class="language-bash">curl -X PUT http://127.0.0.1:8095/api/basestations/4010001 \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "NetCore-Tetra BS 01",
    "short": "BS 01",
    "location": "Hannover",
    "mcc": "901",
    "mnc": "1510",
    "color": "blue",
    "visible": 1,
    "notes": "aktualisierte Beschreibung"
  }'
</code></pre><hr><h2>Basisstation löschen</h2><pre><code class="language-bash">curl -X DELETE http://127.0.0.1:8095/api/basestations/4010001
</code></pre><blockquote><p>Achtung: Vor dem Löschen sollte geprüft werden, ob die ISSI noch in Logs, Configs, Statusrückmeldungen oder Steuerlogik verwendet wird.</p></blockquote><hr><h2>Basisstationen abrufen</h2><p>Alle Basisstationen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/basestations | jq .
</code></pre><p>Eine bestimmte Basisstation:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/basestations/4010001 | jq .
</code></pre><hr><h2>Lookup-kompatible Abfrage</h2><p>Eine Basisstation kann auch über einen Lookup-Endpunkt abgefragt werden.</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/dmr/repeater/?id=4010001' | jq .
</code></pre><p>Beispielantwort:</p><pre><code class="language-json">{
  "count": 1,
  "results": [
    {
      "id": 4010001,
      "callsign": "BS 01",
      "city": "Hannover",
      "state": "NetCore-Tetra BS 01",
      "country": "NetCore"
    }
  ]
}
</code></pre><hr><h2>Import und Export</h2><p>Basisstationen werden im Directory-Export im Bereich <code>basestations</code> gespeichert.</p><p>Beispiel:</p><pre><code class="language-json">{
  "basestations": [
    {
      "issi": 4010001,
      "name": "NetCore-Tetra BS 01",
      "short": "BS 01",
      "location": "Hannover",
      "mcc": "901",
      "mnc": "1510",
      "color": "blue",
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
</code></pre><hr><h2>CSV-Übernahme</h2><p>Wenn Basisstationen aus CSV übernommen werden, sollten die Spalten eindeutig sein.</p><p>Empfohlene CSV-Spalten:</p><pre><code class="language-csv">issi,name,short,location,mcc,mnc,color,visible,notes
4010001,NetCore-Tetra BS 01,BS 01,Hannover,901,1510,blue,1,Haupt-Basisstation
4010002,NetCore-Tetra BS Mobile,BS Mobile,Einsatzkoffer,901,1510,orange,1,Mobile Test-BS
</code></pre><p>Wichtig:</p><pre><code class="language-text">- ISSI als Zahl pflegen
- keine doppelten ISSIs
- Standort einheitlich benennen
- MCC/MNC passend zur Config dokumentieren
- alte Basisstationen mit visible=0 ausblenden
</code></pre><hr><h2>Mehrere Basisstationen</h2><p>NetCore-Tetra kann perspektivisch mehrere Basisstationen dokumentieren oder betreiben.</p><p>Beispiele:</p><pre><code class="language-text">4010001 → BS 01 Hannover
4010002 → BS 02 Mobile
4010003 → BS 03 Campus
</code></pre><p>Für mehrere Basisstationen sollten getrennt gepflegt werden:</p><pre><code class="language-text">- ISSI
- Name
- Standort
- Frequenzbereich
- MCC/MNC
- Service-Name
- Dashboard-Port
- Directory-Anbindung
- SDR-Gerät
</code></pre><p>Die Directory-Seite dokumentiert zunächst die Stammdaten. Die technische Mehrzellenlogik hängt vom jeweiligen Softwarestand ab.</p><hr><h2>Basisstation in der Config</h2><p>Die technische Konfiguration der Basisstation befindet sich nicht im Directory, sondern in der <code>config.toml</code>.</p><p>Relevante Bereiche:</p><pre><code class="language-toml">[net_info]
mcc = 901
mnc = 1510

[cell_info]
freq_band = 4
main_carrier = 720
</code></pre><p>Die Directory-Daten sollten dazu passen.</p><p>Beispiel:</p><pre><code class="language-text">Directory:
4010001, NetCore-Tetra BS 01, MCC 901, MNC 1510

config.toml:
mcc = 901
mnc = 1510
</code></pre><hr><h2>Basisstations-ISSI in der Logik</h2><p>Die Basisstations-ISSI kann in mehreren Stellen verwendet werden:</p><pre><code class="language-text">- SDS-Absender
- Statusrückmeldung
- Dashboard-Systemadresse
- Steuerbefehle
- interne Service-Adresse
</code></pre><p>Wenn diese ISSI geändert wird, müssen alle betroffenen Stellen geprüft werden.</p><p>Typische Prüfpunkte:</p><pre><code class="language-text">- config.toml
- Directory-Basestationseintrag
- SDS-/Statuslogik
- Dashboard-Defaults
- Steuerstatus
- Funkgeräte-Codeplug
</code></pre><hr><h2>Beispiel: Haupt-Basisstation</h2><pre><code class="language-json">{
  "issi": 4010001,
  "name": "NetCore-Tetra BS 01",
  "short": "BS 01",
  "location": "Hannover",
  "mcc": "901",
  "mnc": "1510",
  "color": "blue",
  "visible": 1,
  "notes": "Lokale Haupt-Basisstation"
}
</code></pre><hr><h2>Beispiel: Mobile Basisstation</h2><pre><code class="language-json">{
  "issi": 4010002,
  "name": "NetCore-Tetra BS Mobile",
  "short": "BS Mobile",
  "location": "Einsatzkoffer",
  "mcc": "901",
  "mnc": "1510",
  "color": "orange",
  "visible": 1,
  "notes": "Mobile Test- und Demo-Basisstation"
}
</code></pre><hr><h2>Beispiel: Reserveeintrag</h2><pre><code class="language-json">{
  "issi": 4019999,
  "name": "Reserviert Infrastruktur",
  "short": "Reserve",
  "location": "",
  "mcc": "901",
  "mnc": "1510",
  "color": "gray",
  "visible": 0,
  "notes": "Reservierter Bereich für spätere Infrastruktur"
}
</code></pre><hr><h2>Prüfen, ob die Basisstation korrekt eingetragen ist</h2><p>API-Abfrage:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/basestations/4010001 | jq .
</code></pre><p>Lookup-Abfrage:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/dmr/repeater/?id=4010001' | jq .
</code></pre><p>Dashboard prüfen:</p><pre><code class="language-text">1. Dashboard öffnen
2. SDS- oder Statusereignis erzeugen
3. Prüfen, ob Basisstation lesbar angezeigt wird
4. Logs beobachten
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "4010001|SDS|STATUS|Directory"
</code></pre><hr><h2>Fehleranalyse</h2><h3>Basisstation wird nur als ISSI angezeigt</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Basisstation fehlt im Directory
- ISSI stimmt nicht
- Directory nicht erreichbar
- Lookup-Endpunkt liefert keinen Treffer
- visible ist 0
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/basestations/4010001 | jq .
</code></pre><hr><h3>Statusrückmeldung kommt von falscher ISSI</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- alte Default-ISSI im Code
- Basisstations-ISSI nicht angepasst
- Funkgeräte erwarten andere Ziel-/Quelladresse
- Config und Directory passen nicht zusammen
</code></pre><p>Prüfen:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "HomeModeDisplay|SDS-STATUS|4010001|9999"
</code></pre><p>Wenn noch alte System-ISSIs auftauchen, müssen Config, Code oder Defaults geprüft werden.</p><hr><h3>Directory zeigt Basisstation, Dashboard aber nicht</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Dashboard lädt Basestation-Daten nicht neu
- Cache noch aktiv
- Dashboard-Filter
- falscher Directory-Endpoint
- Basisstation nutzt anderes Directory
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/basestations | jq .
</code></pre><p>und von der Basisstation aus:</p><pre><code class="language-bash">curl -s http://&lt;DIRECTORY-IP&gt;:8095/api/basestations | jq .
</code></pre><hr><h3>Mehrere Einträge wirken gleich</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- doppelte oder ähnliche Namen
- falsche Kurzbezeichnung
- kopierte Einträge ohne Anpassung
- identische Standorte ohne Notiz
</code></pre><p>Empfehlung:</p><pre><code class="language-text">- eindeutiger Name
- eindeutiger Kurzname
- Standort oder Rolle ergänzen
- Notizen nutzen
</code></pre><hr><h2>Best Practices</h2><h3>Basisstations-ISSIs klar trennen</h3><p>Basisstationen sollten in einem eigenen ISSI-Bereich liegen.</p><p>Beispiel:</p><pre><code class="language-text">4010001 Haupt-BS
4010002 Mobile BS
4010003 Campus-BS
</code></pre><p>Nicht empfohlen:</p><pre><code class="language-text">2020001 HRT
2020002 MRT
2020003 Basisstation
</code></pre><p>Eine klare Trennung erleichtert Logs und Fehlersuche.</p><hr><h3>Standort dokumentieren</h3><p>Auch bei Testsystemen lohnt sich ein Standortfeld.</p><p>Beispiele:</p><pre><code class="language-text">Hannover
Technikraum
Mobile Box
Labor
Campus
</code></pre><p>Später kann daraus eine Standort- oder Kartenansicht entstehen.</p><hr><h3>Kurzname kompakt halten</h3><p>Gute Kurznamen:</p><pre><code class="language-text">BS 01
BS Mobile
BS Campus
</code></pre><p>Zu lange Kurznamen machen Dashboard-Tabellen unübersichtlich.</p><hr><h3>Directory und Config synchron halten</h3><p>Wenn MCC, MNC oder Basisstations-ISSI geändert werden, sollten immer beide Stellen geprüft werden:</p><pre><code class="language-text">- config.toml
- Directory
</code></pre><hr><h3>Alte Einträge nicht sofort löschen</h3><p>Bei Tests ist es oft sinnvoll, alte Basisstationen zunächst auszublenden statt zu löschen.</p><pre><code class="language-json">"visible": 0
</code></pre><p>So bleiben Notizen und Historie erhalten.</p><hr><h2>Empfohlener Prüfablauf nach Änderungen</h2><pre><code class="language-text">1. Basisstation im Directory bearbeiten
2. Directory-Eintrag per API prüfen
3. Basisstation neu starten, falls technische ISSI geändert wurde
4. Dashboard öffnen
5. SDS-/Statusereignis erzeugen
6. Logs prüfen
</code></pre><p>API:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/basestations/4010001 | jq .
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "4010001|Directory|SDS|STATUS"
</code></pre><hr><h2>Zusammenhang mit anderen Wiki-Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[NetCore-Directory]]</p></li><li><p>[[Configuration]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[SDS]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Systemd-Service]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>