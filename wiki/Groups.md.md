<html><body>
<!--StartFragment--><html><head></head><body><h1>Groups</h1><p>Diese Seite beschreibt die Verwaltung von GSSI-Gruppen im NetCore Directory.</p><p>GSSI-Gruppen sind die Gruppenadressen im TETRA-Netz. Sie werden für Gruppenrufe, Gruppenanbindungen und die taktische oder organisatorische Struktur des Funkbetriebs genutzt.</p><p>Ohne Directory sieht eine Gruppe nur wie eine technische Nummer aus:</p><pre><code class="language-text">15201
</code></pre><p>Mit Directory wird daraus eine lesbare Gruppe:</p><pre><code class="language-text">15201 → Betriebsgruppe
</code></pre><hr><h2>Aufgabe der Gruppenverwaltung</h2><p>Die Gruppenverwaltung dient dazu, GSSI-Adressen eindeutig und verständlich zu dokumentieren.</p><p>Sie wird genutzt für:</p><ul><li><p>Dashboard-Anzeige,</p></li><li><p>Gruppenanbindungen,</p></li><li><p>Gruppenrufe,</p></li><li><p>SDS-Logs,</p></li><li><p>spätere Gruppenübersichten,</p></li><li><p>Fehlersuche,</p></li><li><p>Import und Export von Stammdaten,</p></li><li><p>klare Trennung von Betriebs-, Technik-, Test- und Sondergruppen.</p></li></ul><p>Die GSSI bleibt dabei der technische Schlüssel. Name, Kurzname, Typ, Besitzer, Farbe und Notizen sind Stammdaten aus dem Directory.</p><hr><h2>Was ist eine GSSI?</h2><p>Eine GSSI ist eine Gruppenadresse im TETRA-Netz.</p><p>Während eine ISSI ein einzelnes Gerät beschreibt, beschreibt eine GSSI eine Gruppe.</p><pre><code class="language-text">ISSI = einzelner Teilnehmer
GSSI = Gruppe / Talkgroup
</code></pre><p>Beispiel:</p><pre><code class="language-text">ISSI 2020001 → HRT Fahrer
GSSI 15201   → Betriebsgruppe
</code></pre><p>Ein Funkgerät kann sich einer oder mehreren Gruppen zuordnen und dort Gruppenrufe empfangen oder senden.</p><hr><h2>GSSI und Gerätegruppen unterscheiden</h2><p>GSSI-Gruppen und Gerätegruppen sind zwei verschiedene Dinge.</p>
Begriff | Bedeutung
-- | --
GSSI-Gruppe | Funkgruppe / Talkgroup im TETRA-Netz
Gerätegruppe | logische Zusammenfassung mehrerer ISSIs, z. B. Fahrzeug
Statusgruppe | Gerätegruppe mit aktivem Status-Sync

<p>Die konkrete Darstellung hängt vom Dashboard ab.</p><hr><h2>Gruppen im Dashboard</h2><p>Das Dashboard kann GSSI-Daten nutzen, um Gruppenanbindungen und Rufe lesbar darzustellen.</p><p>Ohne Directory:</p><pre><code class="language-text">2020001 affiliated to 15201
</code></pre><p>Mit Directory:</p><pre><code class="language-text">HRT Fahrer affiliated to Betriebsgruppe
</code></pre><p>Auch Gruppenrufe werden dadurch verständlicher:</p><pre><code class="language-text">HRT Fahrer → Betriebsgruppe
</code></pre><p>statt:</p><pre><code class="language-text">2020001 → 15201
</code></pre><hr><h2>Gruppenanbindung / Affiliation</h2><p>Wenn ein Funkgerät einer Gruppe beitritt, entsteht ein Affiliation-Ereignis.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät
   │
   │ Affiliation zu GSSI 15201
   ▼
Basisstation
   │
   ├── Gruppenanbindung speichern
   ├── GSSI im Directory auflösen
   └── Dashboard aktualisieren
</code></pre><p>Beispiel:</p><pre><code class="language-text">2020001 meldet sich auf GSSI 15201 an
→ Directory: 15201 = Betriebsgruppe
→ Dashboard: HRT Fahrer → Betriebsgruppe
</code></pre><hr><h2>Gruppenrufe</h2><p>Ein Gruppenruf richtet sich nicht an ein einzelnes Gerät, sondern an alle Teilnehmer einer Gruppe.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät sendet Gruppenruf
   │
   ▼
Basisstation erkennt Ziel-GSSI
   │
   ▼
Basisstation verteilt Ruf an passende Teilnehmer
   │
   ▼
Dashboard zeigt Gruppenruf mit lesbarem Gruppennamen
</code></pre><p>Beispiel:</p><pre><code class="language-text">HRT Fahrer spricht auf Betriebsgruppe
</code></pre><p>Technisch:</p><pre><code class="language-text">ISSI 2020001 → GSSI 15201
</code></pre><hr><h2>GSSI im Codeplug</h2><p>Die GSSI muss nicht nur im Directory existieren, sondern auch in den Funkgeräten korrekt programmiert sein.</p><p>Wichtig:</p><pre><code class="language-text">- GSSI im Funkgerät eintragen
- Gruppenname im Codeplug passend setzen
- Netzparameter müssen stimmen
- Gruppe muss zur Betriebsart passen
- Gerät muss sich auf die Gruppe affiliieren können
</code></pre><p>Das Directory dokumentiert die Gruppe, ersetzt aber nicht die Programmierung der Funkgeräte.</p><hr><h2>GSSI in der Config</h2><p>Je nach Systemstand können bestimmte Gruppen auch in der <code>config.toml</code> relevant sein.</p><p>Typische Bereiche:</p><pre><code class="language-text">- erlaubte Gruppen
- Standardgruppe
- Gruppenrouting
- Gateway-Gruppen
- Testgruppen
</code></pre><p>Die genaue technische Nutzung hängt von der jeweiligen Konfiguration ab.</p><p>Das Directory dient dabei als Stammdatenquelle und Anzeigehilfe.</p><hr><h2>Gruppe hinzufügen</h2><p>Eine Gruppe kann über die Weboberfläche oder per API hinzugefügt werden.</p><h3>Per Weboberfläche</h3><p>Typischer Ablauf:</p><pre><code class="language-text">1. Directory öffnen
2. Tab „Gruppen“ auswählen
3. Neue Gruppe hinzufügen
4. GSSI eintragen
5. Name, Kurzname, Typ und Owner setzen
6. Speichern
</code></pre><hr><h3>Per API</h3><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/groups \
  -H 'Content-Type: application/json' \
  -d '{
    "gssi": 15201,
    "name": "Betriebsgruppe",
    "short": "Betrieb",
    "type": "TMO",
    "owner": "NetCore",
    "color": "blue",
    "visible": 1,
    "notes": "Allgemeine Betriebsgruppe"
  }'
</code></pre><hr><h2>Gruppe bearbeiten</h2><pre><code class="language-bash">curl -X PUT http://127.0.0.1:8095/api/groups/15201 \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "Betriebsgruppe",
    "short": "Betrieb",
    "type": "TMO",
    "owner": "NetCore",
    "color": "blue",
    "visible": 1,
    "notes": "aktualisierte Beschreibung"
  }'
</code></pre><hr><h2>Gruppe löschen</h2><pre><code class="language-bash">curl -X DELETE http://127.0.0.1:8095/api/groups/15201
</code></pre><blockquote><p>Achtung: Vor dem Löschen sollte geprüft werden, ob die GSSI noch in Funkgeräten, Configs, Gateways oder Dokumentation verwendet wird.</p></blockquote><hr><h2>Gruppen abrufen</h2><p>Alle Gruppen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/groups | jq .
</code></pre><p>Eine bestimmte Gruppe:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/groups/15201 | jq .
</code></pre><hr><h2>Import und Export</h2><p>GSSI-Gruppen werden im Directory-Export im Bereich <code>groups</code> gespeichert.</p><p>Beispiel:</p><pre><code class="language-json">{
  "groups": [
    {
      "gssi": 15201,
      "name": "Betriebsgruppe",
      "short": "Betrieb",
      "type": "TMO",
      "owner": "NetCore",
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
</code></pre><hr><h2>CSV-Übernahme</h2><p>Für Gruppenimporte aus CSV sind klare Spalten sinnvoll.</p><p>Empfohlene CSV-Struktur:</p><pre><code class="language-csv">gssi,name,short,type,owner,color,visible,notes
15201,Betriebsgruppe,Betrieb,TMO,NetCore,blue,1,Allgemeine Betriebsgruppe
15202,Technikgruppe,Technik,TMO,NetCore,orange,1,Technik und Tests
15203,Einsatzleitung,EL,TMO,NetCore,green,1,Einsatzleitung
</code></pre><p>Wichtig:</p><pre><code class="language-text">- GSSI als Zahl pflegen
- keine doppelten GSSIs
- Kurzname kompakt halten
- Typen einheitlich verwenden
- alte Gruppen mit visible=0 ausblenden
</code></pre><hr><h2>Doppelte GSSIs vermeiden</h2><p>Jede GSSI darf nur einmal im Directory existieren.</p><p>Problematisch:</p><pre><code class="language-text">15201 → Betriebsgruppe
15201 → Technikgruppe
</code></pre><p>Das führt zu falschen Anzeigen und unklarer Zuordnung.</p><p>Vor Importen sollte geprüft werden:</p><pre><code class="language-text">- Gibt es doppelte GSSIs?
- Wurden alte Gruppen überschrieben?
- Stimmen Namen und Kurzbezeichnungen?
- Sind Testgruppen klar markiert?
</code></pre><hr><h2>Reservierte GSSI-Bereiche</h2><p>Es ist sinnvoll, GSSI-Bereiche zu reservieren.</p><p>Beispiel:</p><pre><code class="language-text">15000–15099  System / Infrastruktur
15100–15199  Testgruppen
15200–15299  Betriebsgruppen
15300–15399  Technikgruppen
15400–15499  Sondergruppen
</code></pre><p>Das konkrete Schema kann frei gewählt werden.</p><p>Wichtig ist, dass es dokumentiert und konsequent eingehalten wird.</p><hr><h2>Beispiel: einfache Gruppenstruktur</h2><pre><code class="language-text">15201 Betriebsgruppe
15202 Technikgruppe
15203 Einsatzleitung
15204 Logistik
15205 Sanitätsdienst
15206 Security
15299 Testgruppe
</code></pre><hr><h2>Beispiel: Event-Betrieb</h2><pre><code class="language-text">15201 Gesamtbetrieb
15202 Security
15203 Medic
15204 Awareness
15205 Einlass
15206 Logistik
15207 Technik
15208 Leitung
</code></pre><p>Jede Gruppe sollte im Directory mit Name, Kurzname, Typ und Owner gepflegt sein.</p><hr><h2>Beispiel: Technik- und Testbetrieb</h2><pre><code class="language-text">15101 SDS-Test
15102 Status-Test
15103 GPS-Test
15104 Telefonie-Test
15105 Gateway-Test
</code></pre><p>Testgruppen sollten klar als <code>Test</code> markiert werden.</p><hr><h2>Gruppen und Gateways</h2><p>Bestimmte Gruppen können für Gateways vorgesehen sein.</p><p>Beispiele:</p><pre><code class="language-text">Telefonie-Gateway-Gruppe
SDS-Gateway-Gruppe
Externe Kopplungsgruppe
Alarmgruppe
</code></pre><p>In solchen Fällen sollte das Feld <code>notes</code> dokumentieren, wofür die Gruppe genutzt wird.</p><p>Beispiel:</p><pre><code class="language-json">{
  "gssi": 15301,
  "name": "Telefonie Gateway",
  "short": "Tel-GW",
  "type": "Gateway",
  "owner": "NetCore",
  "color": "orange",
  "visible": 1,
  "notes": "Gruppe für Telefonie-Kopplung"
}
</code></pre><hr><h2>Gruppen und Statuslogik</h2><p>Statusmeldungen werden grundsätzlich von einzelnen ISSIs gesendet.</p><p>Eine GSSI ist nicht automatisch eine Statusgruppe.</p><p>Wichtig:</p><pre><code class="language-text">GSSI-Gruppe ≠ Statusgruppe
</code></pre><p>Beispiel:</p><pre><code class="language-text">GSSI 15201 = Betriebsgruppe
Statusgruppe RTW 83-01 = 2020001, 2020002, 2020003
</code></pre><p>Die GSSI beschreibt die Funkgruppe.<br>Die Statusgruppe beschreibt, welche Geräte gemeinsam denselben Status tragen.</p><hr><h2>Gruppen und SDS</h2><p>SDS kann je nach Zieladresse an einzelne Geräte oder Gruppen gehen.</p><p>Typische Zielarten:</p><pre><code class="language-text">ISSI → einzelnes Gerät
GSSI → Gruppe
Systemadresse → Basisstation oder Dienst
</code></pre><p>Wenn SDS an eine GSSI gesendet oder dort geloggt wird, kann das Directory den Gruppennamen anzeigen.</p><hr><h2>Prüfen, ob eine Gruppe korrekt eingetragen ist</h2><p>API-Abfrage:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/groups/15201 | jq .
</code></pre><p>Dashboard prüfen:</p><pre><code class="language-text">1. Funkgerät auf Gruppe schalten
2. Registrierung / Affiliation abwarten
3. Dashboard öffnen
4. Prüfen, ob Gruppenname statt nur GSSI angezeigt wird
5. Gruppenruf oder SDS-Test auslösen
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "15201|Affiliate|Group|GSSI|call"
</code></pre><hr><h2>Fehleranalyse</h2><h3>Gruppe wird nur als Nummer angezeigt</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gruppe fehlt im Directory
- GSSI falsch eingetragen
- Directory nicht erreichbar
- Dashboard nutzt Cache
- visible ist 0
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/groups/15201 | jq .
</code></pre><hr><h3>Gerät kann sich nicht auf Gruppe affiliieren</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- GSSI fehlt im Funkgerät-Codeplug
- falsche Netzkennung
- falsche Gruppe im Gerät ausgewählt
- Gruppenrouting nicht aktiv
- Uplink/Downlink-Probleme
- Gerät nicht korrekt registriert
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "Affiliate|Register|15201|2020001"
</code></pre><hr><h3>Gruppenruf kommt nicht an</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Zielgeräte sind nicht registriert
- Zielgeräte sind nicht auf der Gruppe affiliiert
- falsche GSSI
- Codeplug passt nicht
- Gruppenrufrouting fehlerhaft
- Gerät befindet sich im Ruhezustand
</code></pre><p>Prüfen:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "Group call|GSSI|15201|2020001|2020002"
</code></pre><hr><h3>Falscher Gruppenname im Dashboard</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- falscher Directory-Eintrag
- GSSI vertauscht
- alter Cache
- Import hat Gruppe überschrieben
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/groups | jq '.[] | select(.gssi == 15201)'
</code></pre><hr><h2>Best Practices</h2><h3>GSSI-Bereiche sauber planen</h3><p>Vor dem Anlegen vieler Gruppen sollte ein Nummernplan erstellt werden.</p><p>Beispiel:</p><pre><code class="language-text">151xx = Test
152xx = Betrieb
153xx = Technik
154xx = Sondergruppen
</code></pre><hr><h3>Kurznamen kurz halten</h3><p>Gute Kurznamen:</p><pre><code class="language-text">Betrieb
Technik
EL
Medic
Security
Logistik
</code></pre><p>Zu lange Kurznamen machen Dashboard-Tabellen unübersichtlich.</p><hr><h3>Testgruppen klar markieren</h3><p>Testgruppen sollten eindeutig erkennbar sein.</p><p>Beispiel:</p><pre><code class="language-json">{
  "gssi": 15101,
  "name": "SDS-Testgruppe",
  "short": "SDS-Test",
  "type": "Test",
  "owner": "NetCore",
  "visible": 1
}
</code></pre><hr><h3>Gruppen nicht für Fahrzeugstatus missbrauchen</h3><p>Fahrzeugstatus sollte über Gerätegruppen / Statusgruppen abgebildet werden, nicht über GSSI-Gruppen.</p><p>Besser:</p><pre><code class="language-text">GSSI 15201 = Betriebsgruppe
Gerätegruppe RTW 83-01 = Fahrzeugstatus
</code></pre><p>Nicht ideal:</p><pre><code class="language-text">GSSI 15201 = RTW 83-01 Statusgruppe
</code></pre><p>Die Trennung macht das System langfristig sauberer.</p><hr><h3>Notizen nutzen</h3><p>Das Feld <code>notes</code> ist hilfreich für:</p><pre><code class="language-text">- Zweck der Gruppe
- zugehörige Geräte
- Gateway-Nutzung
- Testzweck
- historische Hinweise
- Codeplug-Hinweise
</code></pre><hr><h2>Empfohlener Prüfablauf nach Änderungen</h2><pre><code class="language-text">1. Gruppe im Directory bearbeiten
2. API-Abfrage prüfen
3. Funkgerät auf Gruppe schalten
4. Affiliation im Log prüfen
5. Dashboard kontrollieren
6. Gruppenruf testen
</code></pre><p>API:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/groups/15201 | jq .
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "15201|Affiliate|Group|GSSI"
</code></pre><hr><h2>Zusammenhang mit anderen Wiki-Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[NetCore-Directory]]</p></li><li><p>[[Devices]]</p></li><li><p>[[Device-Groups]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[SDS]]</p></li><li><p>[[Group-Calls]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>