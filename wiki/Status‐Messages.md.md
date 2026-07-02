<html><body>
<!--StartFragment--><html><head></head><body><h1>Status Messages</h1><p>Diese Seite beschreibt die Verwaltung von Statusmeldungen im NetCore Directory.</p><p>Statusmeldungen ordnen technische Statuscodes lesbaren Texten zu. Dadurch wird aus einer reinen Statusnummer im Funknetz eine verständliche Anzeige im Dashboard und auf unterstützten Funkgeräten.</p><p>Ohne Directory:</p><pre><code class="language-text">Status 1
</code></pre><p>Mit Directory:</p><pre><code class="language-text">Status 1 → Frei auf Funk
</code></pre><hr><h2>Aufgabe der Statusmeldungen</h2><p>Statusmeldungen dienen dazu, kurze taktische oder betriebliche Zustände zu übertragen.</p><p>Sie werden genutzt für:</p><ul><li><p>Statusanzeige im Dashboard,</p></li><li><p>Statusrückmeldung an Funkgeräte,</p></li><li><p>Statusgruppen / Fahrzeugstatus,</p></li><li><p>Rejoin-Replay,</p></li><li><p>SDS-Log,</p></li><li><p>spätere Fahrzeug-/OPTA-Ansicht,</p></li><li><p>Auswertungen und Betriebsübersicht.</p></li></ul><p>Die Basisstation empfängt technisch nur einen Statuscode. Die Bedeutung dieses Codes kommt aus dem NetCore Directory.</p><hr><h2>Grundprinzip</h2><p>Ein Funkgerät sendet einen Statuscode.</p><pre><code class="language-text">2020001 sendet Status 1
</code></pre><p>Die Basisstation fragt das Directory:</p><pre><code class="language-text">Was bedeutet Status 1?
</code></pre><p>Das Directory antwortet:</p><pre><code class="language-text">Frei auf Funk
</code></pre><p>Danach kann die Basisstation:</p><pre><code class="language-text">- Dashboard aktualisieren
- Status im SDS-Log anzeigen
- HMD-Rückmeldung an das Funkgerät senden
- Status auf Gerätegruppe synchronisieren
- Status für späteren Rejoin speichern
</code></pre><hr><h2>Statuscode</h2><p>Der Statuscode ist der technische Wert, den das Funkgerät sendet.</p><p>Beispiele:</p><pre><code class="language-text">1
2
3
4
5
6
32771
50005
</code></pre><p>Statuscodes können je nach Funkgerät, Codeplug und Betriebslogik frei oder herstellerspezifisch belegt sein.</p><p>Für den Regelbetrieb sollten Statuscodes eindeutig dokumentiert werden.</p><hr><h2>Statusfelder</h2><p>Eine Statusmeldung im Directory kann mehrere Felder enthalten.</p>
Feld | Bedeutung
-- | --
code | technische Statusnummer
label | kurzer lesbarer Statustext
severity | Einordnung für Anzeige oder Priorität
description | längere Beschreibung
color | Farbe für Dashboard oder UI
visible | Sichtbarkeit
notes | freie Notizen, falls vorhanden

<p>Die Farbnamen sollten einheitlich verwendet werden.</p><hr><h2>Sichtbarkeit</h2><p>Das Feld <code>visible</code> legt fest, ob ein Status in Anzeigen oder Listen erscheinen soll.</p><pre><code class="language-json">"visible": 1
</code></pre><p>bedeutet sichtbar.</p><pre><code class="language-json">"visible": 0
</code></pre><p>bedeutet ausgeblendet.</p><p>Ausgeblendete Statusmeldungen können sinnvoll sein für:</p><ul><li><p>interne Steuerstatus,</p></li><li><p>alte Statuscodes,</p></li><li><p>Teststatus,</p></li><li><p>herstellerspezifische Sonderstatus,</p></li><li><p>reservierte Codes.</p></li></ul><hr><h2>Statusmeldungen im Dashboard</h2><p>Das Dashboard nutzt Statusmeldungen aus dem Directory, um Statuscodes lesbar anzuzeigen.</p><p>Ohne Directory:</p><pre><code class="language-text">2020001 → Status 1
</code></pre><p>Mit Directory:</p><pre><code class="language-text">HRT Fahrer → Frei auf Funk
</code></pre><p>Zusätzlich können angezeigt werden:</p><pre><code class="language-text">- Farbe
- Severity
- Zeitstempel
- Quelle
- Statusgruppe
- Beschreibung
</code></pre><hr><h2>Statusrückmeldung an Funkgeräte</h2><p>Nach Empfang einer Statusmeldung kann die Basisstation eine lesbare Rückmeldung an das Funkgerät senden.</p><p>Beispiel:</p><pre><code class="language-text">Status: Frei auf Funk
</code></pre><p>Ablauf:</p><pre><code class="language-text">Funkgerät sendet Status 1
   │
   ▼
Basisstation empfängt U-STATUS
   │
   ▼
Directory liefert Label „Frei auf Funk“
   │
   ▼
Basisstation sendet Display-Rückmeldung
</code></pre><p>Dadurch sieht das Funkgerät nicht nur eine technische Statusnummer, sondern einen verständlichen Text.</p><hr><h2>Statusgruppen</h2><p>Statusmeldungen können auf Gerätegruppen synchronisiert werden.</p><p>Beispiel:</p><pre><code class="language-text">Gerätegruppe RTW 83-01
├── 2020001 HRT Fahrer
├── 2020002 MRT Fahrzeug
└── 2020003 HRT Beifahrer
</code></pre><p>Wenn <code>2020001</code> Status 1 sendet:</p><pre><code class="language-text">2020001 → Frei auf Funk
2020002 → Frei auf Funk
2020003 → Frei auf Funk
</code></pre><p>Die Statusmeldung selbst kommt aus dem Directory. Die Gruppenzuordnung kommt ebenfalls aus dem Directory.</p><hr><h2>Rejoin-Replay</h2><p>Wenn ein Gerät offline ist, kann die Basisstation den letzten bekannten Status speichern.</p><p>Sobald das Gerät erneut registriert, kann der Status erneut zugestellt werden.</p><p>Ablauf:</p><pre><code class="language-text">1. Gerät 2020002 ist offline
2. Statusgruppe erhält Status „Frei auf Funk“
3. Basisstation speichert Status für 2020002
4. Gerät 2020002 registriert später neu
5. Basisstation sendet Status erneut an 2020002
</code></pre><p>Das sorgt dafür, dass Geräte nach einem Neustart oder Rejoin wieder den aktuellen Status erhalten.</p><hr><h2>Live Directory Sync</h2><p>Wenn Statusgruppen im Directory geändert werden, kann die Basisstation die Gruppenzuordnung regelmäßig neu laden.</p><p>Beispiel:</p><pre><code class="language-text">1. 2020001 sendet Status „Frei auf Funk“
2. Gruppe enthält 2020001 und 2020002
3. Beide bekommen den Status
4. Im Directory werden 2020003 und 2020004 ergänzt
5. Basisstation zieht die Gruppe neu
6. 2020003 und 2020004 übernehmen den vorhandenen Status
</code></pre><p>Dadurch muss nicht erneut ein Status gesendet werden, nur damit neue Gruppenmitglieder den aktuellen Stand erhalten.</p><hr><h1>API</h1><h2>Alle Statusmeldungen abrufen</h2><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status | jq .
</code></pre><p>Beispielantwort:</p><pre><code class="language-json">[
  {
    "code": 1,
    "label": "Frei auf Funk",
    "severity": "ok",
    "description": "Fahrzeug oder Gerät ist frei und erreichbar.",
    "color": "green",
    "visible": 1
  },
  {
    "code": 5,
    "label": "Sprechwunsch",
    "severity": "warn",
    "description": "Teilnehmer möchte sprechen.",
    "color": "orange",
    "visible": 1
  }
]
</code></pre><hr><h2>Einzelnen Status abrufen</h2><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status/1 | jq .
</code></pre><p>Beispielantwort:</p><pre><code class="language-json">{
  "code": 1,
  "label": "Frei auf Funk",
  "severity": "ok",
  "description": "Fahrzeug oder Gerät ist frei und erreichbar.",
  "color": "green",
  "visible": 1
}
</code></pre><hr><h2>Status hinzufügen</h2><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/status \
  -H 'Content-Type: application/json' \
  -d '{
    "code": 1,
    "label": "Frei auf Funk",
    "severity": "ok",
    "description": "Fahrzeug oder Gerät ist frei und erreichbar.",
    "color": "green",
    "visible": 1
  }'
</code></pre><hr><h2>Status bearbeiten</h2><pre><code class="language-bash">curl -X PUT http://127.0.0.1:8095/api/status/1 \
  -H 'Content-Type: application/json' \
  -d '{
    "label": "Frei auf Funk",
    "severity": "ok",
    "description": "Fahrzeug oder Gerät ist frei und über Funk erreichbar.",
    "color": "green",
    "visible": 1
  }'
</code></pre><hr><h2>Status löschen</h2><pre><code class="language-bash">curl -X DELETE http://127.0.0.1:8095/api/status/1
</code></pre><blockquote><p>Achtung: Vor dem Löschen sollte geprüft werden, ob der Status noch von Funkgeräten, Codeplugs, Statusgruppen oder Steuerlogik verwendet wird.</p></blockquote><hr><h1>Import und Export</h1><p>Statusmeldungen werden im Directory-Export im Bereich <code>status_messages</code> gespeichert.</p><p>Beispiel:</p><pre><code class="language-json">{
  "status_messages": [
    {
      "code": 1,
      "label": "Frei auf Funk",
      "severity": "ok",
      "description": "Fahrzeug oder Gerät ist frei und erreichbar.",
      "color": "green",
      "visible": 1
    }
  ]
}
</code></pre><p>Export:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/export \
  -o netcore-directory-export.json
</code></pre><p>Import:</p><pre><code class="language-bash">curl -X POST http://127.0.0.1:8095/api/import \
  -H 'Content-Type: application/json' \
  --data-binary @netcore-directory-export.json
</code></pre><hr><h2>CSV-Übernahme</h2><p>Für Statusmeldungen aus CSV ist eine klare Struktur hilfreich.</p><p>Empfohlene CSV-Spalten:</p><pre><code class="language-csv">code,label,severity,description,color,visible
1,Frei auf Funk,ok,Fahrzeug oder Gerät ist frei und erreichbar.,green,1
2,Einsatzbereit,ok,Fahrzeug oder Gerät ist einsatzbereit.,green,1
3,Auftrag übernommen,info,Auftrag wurde übernommen.,blue,1
4,Ankunft,info,Ankunft am Ziel oder Einsatzort.,blue,1
5,Sprechwunsch,warn,Teilnehmer möchte sprechen.,orange,1
6,Nicht einsatzbereit,danger,Fahrzeug oder Gerät ist nicht einsatzbereit.,red,1
</code></pre><p>Wichtig:</p><pre><code class="language-text">- Statuscode als Zahl pflegen
- keine doppelten Codes
- Labels kurz halten
- Severity einheitlich verwenden
- Farben konsistent nutzen
</code></pre><hr><h2>Doppelte Statuscodes vermeiden</h2><p>Jeder Statuscode darf nur einmal existieren.</p><p>Problematisch:</p><pre><code class="language-text">1 → Frei auf Funk
1 → Einsatzbereit
</code></pre><p>Das führt zu falscher Anzeige und unklarer Rückmeldung.</p><p>Vor Importen sollte geprüft werden:</p><pre><code class="language-text">- gibt es doppelte Codes?
- wurden alte Codes überschrieben?
- stimmen Labels und Beschreibungen?
- sind Steuerstatus klar getrennt?
</code></pre><hr><h2>Reservierte Statuscodes</h2><p>Es ist sinnvoll, Statuscode-Bereiche zu reservieren.</p><p>Beispiel:</p><pre><code class="language-text">1–99       reguläre Betriebsstatus
100–199    organisatorische Status
200–299    technische Status
50000+     Steuer- und Systemstatus
</code></pre><p>Das konkrete Schema kann frei gewählt werden.</p><p>Wichtig ist, dass Steuerstatus nicht versehentlich als normaler Betriebsstatus genutzt werden.</p><hr><h2>Steuerstatus</h2><p>Einige Statuscodes können für Steuerbefehle genutzt werden.</p><p>Beispiel:</p><pre><code class="language-text">50005 → Remote Restart
</code></pre><p>Solche Statuscodes sollten besonders vorsichtig gepflegt werden.</p><p>Empfehlungen:</p><pre><code class="language-text">- klar als Systemstatus markieren
- visible optional auf 0 setzen
- nicht als normalen Betriebsstatus verwenden
- nur bekannte Quell-ISSIs zulassen
- Logs beobachten
</code></pre><p>Beispiel:</p><pre><code class="language-json">{
  "code": 50005,
  "label": "Remote Restart",
  "severity": "system",
  "description": "Systemstatus zur Auslösung eines Neustarts.",
  "color": "purple",
  "visible": 0
}
</code></pre><hr><h2>Hersteller- und Sonderstatus</h2><p>Manche Funkgeräte senden Sonderstatus oder proprietäre Statuscodes.</p><p>Diese sollten nicht vorschnell als reguläre Betriebsstatus interpretiert werden.</p><p>Empfehlung:</p><pre><code class="language-text">1. Status im Log beobachten
2. Quelle und Ziel prüfen
3. Bedeutung verifizieren
4. erst dann im Directory dokumentieren
</code></pre><p>Beispiel für einen Sonderstatus:</p><pre><code class="language-json">{
  "code": 32771,
  "label": "Geräte-Sonderstatus",
  "severity": "system",
  "description": "Gerätespezifischer Status. Bedeutung abhängig vom Gerätetyp.",
  "color": "gray",
  "visible": 0
}
</code></pre><hr><h2>Status und Codeplug</h2><p>Die Statuscodes im Directory müssen zum Codeplug der Funkgeräte passen.</p><p>Prüfpunkte:</p><pre><code class="language-text">- Statusnummer im Funkgerät
- angezeigter Name im Funkgerät
- Bedeutung im Directory
- erwartete HMD-Rückmeldung
- Verhalten bei Statusgruppen
</code></pre><p>Wenn Funkgerät und Directory unterschiedliche Bedeutungen verwenden, entstehen falsche Anzeigen.</p><p>Beispielproblem:</p><pre><code class="language-text">Funkgerät:
Status 1 = Einsatzbereit

Directory:
Status 1 = Frei auf Funk
</code></pre><p>Das sollte vermieden werden.</p><hr><h2>Status und Dashboard</h2><p>Das Dashboard zeigt den letzten bekannten Status pro Gerät.</p><p>Mögliche Anzeige:</p><pre><code class="language-text">HRT Fahrer       ONLINE       Frei auf Funk
MRT RTW 83-01    ONLINE       Frei auf Funk
HRT Beifahrer    RUHEMODUS    Frei auf Funk
</code></pre><p>Wenn ein Status über eine Statusgruppe synchronisiert wurde, können mehrere Geräte denselben Status anzeigen, obwohl nur eines den Status gesendet hat.</p><hr><h2>Status und SDS-Log</h2><p>Statusereignisse können zusätzlich im SDS-Log erscheinen.</p><p>Beispiel:</p><pre><code class="language-text">2020001 → 4010001: Status: Frei auf Funk
4010001 → 2020001: Status: Frei auf Funk
</code></pre><p>Das SDS-Log hilft bei der Fehlersuche, weil man dort Quelle, Ziel, Richtung und Zeitverlauf sieht.</p><hr><h2>Status und HMD</h2><p>Home Mode Display wird genutzt, um den lesbaren Status an Funkgeräte zurückzumelden.</p><p>Beispiel:</p><pre><code class="language-text">Status: Frei auf Funk
</code></pre><p>Nicht jedes Gerät zeigt solche Rückmeldungen gleich an. Je nach Gerätetyp, Codeplug oder Firmware kann die Anzeige unterschiedlich ausfallen.</p><hr><h2>Status-Throttle</h2><p>Damit Funkgeräte nicht mit Rückmeldungen überschüttet werden, kann die Basisstation Statusrückmeldungen drosseln.</p><p>Beispiel:</p><pre><code class="language-text">Status 1 von 2020001 empfangen
→ HMD wird gesendet

Status 1 von 2020001 direkt erneut empfangen
→ HMD wird eventuell unterdrückt
</code></pre><p>Das verhindert Antwort-Spam bei mehrfachen oder wiederholten Statussendungen.</p><hr><h2>Prüfen, ob Statusmeldungen funktionieren</h2><p>Statusliste abrufen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status | jq .
</code></pre><p>Einzelnen Status abrufen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status/1 | jq .
</code></pre><p>Basisstationslogs beobachten:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "SDS-STATUS|HomeModeDisplay|Status"
</code></pre><p>Status vom Funkgerät senden und prüfen:</p><pre><code class="language-text">1. Funkgerät registrieren
2. Status senden
3. Dashboard prüfen
4. HMD-Rückmeldung prüfen
5. Logs beobachten
</code></pre><hr><h2>Fehleranalyse</h2><h3>Status wird nur als Nummer angezeigt</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Statuscode fehlt im Directory
- Directory ist nicht erreichbar
- Statuscode ist anders als erwartet
- Cache noch nicht aktualisiert
- Status ist visible=0
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status/&lt;CODE&gt; | jq .
</code></pre><hr><h3>HMD-Rückmeldung kommt nicht an</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Gerät ist nicht registriert
- Statuscode fehlt im Directory
- HMD wird vom Gerät nicht angezeigt
- falsche Ziel-ISSI
- Rückmeldung wurde durch Throttle begrenzt
- SDS-Zustellung fehlgeschlagen
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "HomeModeDisplay|SDS-STATUS|&lt;ISSI&gt;"
</code></pre><hr><h3>Falscher Statustext</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Codeplug und Directory stimmen nicht überein
- Statuscode wurde falsch importiert
- alter Directory-Export wurde eingespielt
- Statuscode doppelt gepflegt
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/status | jq '.[] | select(.code == 1)'
</code></pre><hr><h3>Statusgruppe übernimmt Status nicht</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Statuscode existiert, aber Gerät ist nicht in Statusgruppe
- status_sync ist deaktiviert
- Directory-Gruppenabfrage liefert nur die sendende ISSI
- Basisstation nutzt falsche Directory-URL
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><hr><h3>Neuer Gruppenstatus erscheint nicht sofort</h3><p>Mögliche Ursachen:</p><pre><code class="language-text">- Directory-Cache noch nicht abgelaufen
- Live Directory Sync läuft nicht
- Gerät hat noch keinen bekannten Statuscache
- Gruppe wurde nicht gespeichert
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "status-sync|Directory refresh|SDS-STATUS"
</code></pre><hr><h2>Best Practices</h2><h3>Statuscodes dokumentieren</h3><p>Jeder produktive Statuscode sollte dokumentiert sein.</p><p>Empfohlen:</p><pre><code class="language-text">Code
Label
Bedeutung
Farbe
Severity
Verwendung
</code></pre><hr><h3>Labels kurz halten</h3><p>Gute Labels:</p><pre><code class="language-text">Frei auf Funk
Einsatzbereit
Sprechwunsch
Nicht einsatzbereit
</code></pre><p>Zu lange Labels können auf Funkgeräten abgeschnitten werden.</p><hr><h3>Steuerstatus trennen</h3><p>Steuerstatus sollten nicht zwischen normalen Betriebsstatus stehen.</p><p>Besser:</p><pre><code class="language-text">1–99 normale Status
50000+ Steuerstatus
</code></pre><hr><h3>Vor Importen exportieren</h3><p>Vor größeren Änderungen:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/export \
  -o netcore-directory-before-status-import-$(date +%F-%H%M).json
</code></pre><hr><h3>Codeplug und Directory gemeinsam pflegen</h3><p>Wenn Statuscodes im Funkgerät geändert werden, sollte direkt das Directory geprüft werden.</p><p>Checkliste:</p><pre><code class="language-text">- Statusnummer gleich?
- Label gleich?
- Bedeutung gleich?
- HMD-Text sinnvoll?
- Dashboard-Farbe passend?
</code></pre><hr><h2>Beispiel: vollständige Statusliste</h2><pre><code class="language-json">[
  {
    "code": 1,
    "label": "Frei auf Funk",
    "severity": "ok",
    "description": "Fahrzeug oder Gerät ist frei und über Funk erreichbar.",
    "color": "green",
    "visible": 1
  },
  {
    "code": 2,
    "label": "Einsatzbereit",
    "severity": "ok",
    "description": "Fahrzeug oder Gerät ist einsatzbereit.",
    "color": "green",
    "visible": 1
  },
  {
    "code": 3,
    "label": "Auftrag übernommen",
    "severity": "info",
    "description": "Ein Auftrag wurde übernommen.",
    "color": "blue",
    "visible": 1
  },
  {
    "code": 4,
    "label": "Ankunft",
    "severity": "info",
    "description": "Ankunft am Ziel oder Einsatzort.",
    "color": "blue",
    "visible": 1
  },
  {
    "code": 5,
    "label": "Sprechwunsch",
    "severity": "warn",
    "description": "Teilnehmer möchte sprechen.",
    "color": "orange",
    "visible": 1
  },
  {
    "code": 6,
    "label": "Nicht einsatzbereit",
    "severity": "danger",
    "description": "Fahrzeug oder Gerät ist nicht einsatzbereit.",
    "color": "red",
    "visible": 1
  },
  {
    "code": 50005,
    "label": "Remote Restart",
    "severity": "system",
    "description": "Systemstatus zur Auslösung eines Neustarts.",
    "color": "purple",
    "visible": 0
  }
]
</code></pre><hr><h2>Zusammenhang mit anderen Wiki-Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[NetCore-Directory]]</p></li><li><p>[[Devices]]</p></li><li><p>[[Device-Groups]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Live-Directory-Sync]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>