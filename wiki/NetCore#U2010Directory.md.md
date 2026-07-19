<html><body>
<!--StartFragment--><html><head></head><body><h1>NetCore Directory</h1><p>Das NetCore Directory ist die zentrale Stammdatenverwaltung von NetCore-Tetra.</p><p>Es speichert und verwaltet die Informationen, die aus technischen Funk-IDs lesbare Betriebsdaten machen. Die Basisstation nutzt das Directory, um Geräte, Basisstationen, Gruppen, Statusmeldungen und Fahrzeug-/Statusgruppen aufzulösen.</p><p>Ohne Directory sieht die Basisstation vor allem Nummern:</p><pre><code class="language-text">ISSI 2020001
GSSI 15201
Status 1
</code></pre><p>Mit Directory werden daraus verständliche Informationen:</p><pre><code class="language-text">2020001 → HRT Fahrer
15201   → Betriebsgruppe
Status 1 → Frei auf Funk
</code></pre><hr><h2>Aufgaben des Directory</h2><p>Das Directory übernimmt mehrere Aufgaben:</p><ul><li><p>Geräte verwalten,</p></li><li><p>Basisstationen verwalten,</p></li><li><p>GSSI-Gruppen verwalten,</p></li><li><p>Statusmeldungen verwalten,</p></li><li><p>Gerätegruppen / Fahrzeuggruppen verwalten,</p></li><li><p>Statusgruppen für synchronisierte Statuslogik bereitstellen,</p></li><li><p>Daten per Weboberfläche pflegen,</p></li><li><p>Daten per API bereitstellen,</p></li><li><p>Import und Export ermöglichen.</p></li></ul><p>Das Directory ist bewusst als eigener Dienst getrennt von der Basisstation aufgebaut. Dadurch können Stammdaten geändert werden, ohne die Basisstation neu bauen oder hart konfigurieren zu müssen.</p><hr><h2>Grundprinzip</h2><p>Die Basisstation verarbeitet Funkereignisse. Das Directory liefert dazu die passenden Namen und Zuordnungen.</p><p>Beispiel:</p><pre><code class="language-text">Funkgerät sendet Status 1
   │
   ▼
Basisstation empfängt U-STATUS
   │
   ▼
Basisstation fragt Directory:
Was bedeutet Status 1?
   │
   ▼
Directory antwortet:
Frei auf Funk
   │
   ▼
Dashboard und Funkgerät zeigen:
Status: Frei auf Funk
</code></pre><hr><h2>Komponenten</h2><p>Das Directory besteht aus:</p>
Komponente | Aufgabe
-- | --
Python-Server | stellt Web-UI und API bereit
SQLite-Datenbank | speichert Geräte, Gruppen und Statusdaten
Weboberfläche | Pflege der Directory-Daten
JSON Import/Export | Sicherung und Übernahme von Daten
API-Endpunkte | Abfrage durch Basisstation und Dashboard

<hr><h1>Troubleshooting</h1><h2>Directory läuft nicht</h2><p>Prüfen:</p><pre><code class="language-bash">sudo systemctl status netcore-directory.service
sudo journalctl -u netcore-directory.service -n 100
</code></pre><p>Manuell starten:</p><pre><code class="language-bash">python3 netcore_directory_server.py --host 0.0.0.0 --port 8095 --db ./netcore_directory.db
</code></pre><hr><h2>Directory antwortet nicht</h2><p>Prüfen:</p><pre><code class="language-bash">curl -v http://127.0.0.1:8095/api/health
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Dienst läuft nicht
- falscher Port
- Firewall
- falsche IP
- Dienst nur auf 127.0.0.1 gebunden
</code></pre><hr><h2>Basisstation bekommt keine Namen</h2><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/devices | jq .
</code></pre><p>Dann von der Basisstation aus:</p><pre><code class="language-bash">curl -s http://&lt;DIRECTORY-IP&gt;:8095/api/devices | jq .
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Directory-Anbindung deaktiviert
- falsche base_url
- Gerät nicht eingetragen
- ISSI stimmt nicht
- visible = 0
</code></pre><hr><h2>Status wird nicht als Text angezeigt</h2><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status | jq .
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Statuscode fehlt im Directory
- Statuscode anders als erwartet
- Directory nicht erreichbar
- Cache noch nicht aktualisiert
</code></pre><hr><h2>Statusgruppe greift nicht</h2><p>Prüfen:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Erwartet:</p><pre><code class="language-json">{
  "status_sync_members": [
    2020001,
    2020002
  ]
}
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät ist nicht Mitglied der Gruppe
- status_sync ist deaktiviert
- ISSI falsch eingetragen
- Mitgliederliste enthält Text statt Zahlen
- Gruppe ist nicht sichtbar oder nicht korrekt gespeichert
</code></pre><hr><h2>Änderungen werden nicht sofort übernommen</h2><p>Die Basisstation cached Directory-Daten kurzzeitig.</p><p>Prüfen:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "Directory|status-sync|SDS-STATUS"
</code></pre><p>Mögliche Ursachen:</p><pre><code class="language-text">- Cache-Intervall noch nicht abgelaufen
- Directory antwortet fehlerhaft
- Basisstation nutzt andere Directory-URL
- Gruppe wurde gespeichert, aber Mitgliederliste ist leer oder ungültig
</code></pre><hr><h1>Empfehlungen</h1><h2>Saubere Namensgebung</h2><p>Empfohlen:</p><pre><code class="language-text">Gerätename: HRT Fahrer
Kurzname: Fahrer
Typ: HRT
Rolle: Fahrzeugbesatzung
Owner: NetCore
</code></pre><p>Für Fahrzeuge:</p><pre><code class="language-text">Name: RTW 83-01
Kurzname: 83-01
OPTA: RK H 83-01
Typ: vehicle
</code></pre><hr><h2>Statuscodes einheitlich halten</h2><p>Statuscodes sollten klar und konsistent gepflegt werden.</p><p>Beispiel:</p><pre><code class="language-text">1 = Frei auf Funk
2 = Einsatzbereit
3 = Auftrag übernommen
4 = Ankunft
5 = Sprechwunsch
6 = Nicht einsatzbereit
</code></pre><p>Die tatsächliche Belegung kann frei gewählt werden, sollte aber dokumentiert bleiben.</p><hr><h2>Regelmäßig exportieren</h2><p>Empfohlen:</p><pre><code class="language-text">nach größeren Änderungen
vor Updates
vor Importen
nach neuen Gerätegruppen
nach Statuslisten-Änderungen
</code></pre><p>Export:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/export \
  -o netcore-directory-export-$(date +%F-%H%M).json
</code></pre><hr><h2>Nächste Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[Devices]]</p></li><li><p>[[Basestations]]</p></li><li><p>[[Groups]]</p></li><li><p>[[Status-Messages]]</p></li><li><p>[[Device-Groups]]</p></li><li><p>[[Directory-API]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>