<html><body>
<!--StartFragment--><html><head></head><body><h1>Architektur</h1><p>Diese Seite beschreibt den grundsätzlichen Aufbau von NetCore-Tetra, die beteiligten Komponenten und die wichtigsten Datenflüsse zwischen Basisstation, Dashboard, Directory, Funkgeräten und optionalen Schnittstellen.</p><p>Ziel dieser Seite ist ein technischer Überblick: Wer spricht mit wem, welche Komponente ist wofür zuständig und wo entstehen welche Betriebsdaten?</p><hr><h2>Gesamtbild</h2><p>NetCore-Tetra ist modular aufgebaut. Die Basisstation bildet den funktechnischen Kern. Das Dashboard stellt den Betriebszustand dar. Das NetCore Directory liefert Stammdaten, Statusnamen, Gruppeninformationen und Fahrzeug-/Gerätezuordnungen.</p><p>Vereinfacht:</p><pre><code class="language-text">Funkgeräte
   │
   │ TETRA Air Interface
   ▼
Basisstation
   │
   ├── Dashboard
   │
   ├── NetCore Directory
   │
   ├── SDS / Statuslogik
   │
   ├── GPS / LIP Verarbeitung
   │
   └── optionale Gateways
</code></pre><p>Die Basisstation bleibt dabei die zentrale aktive Komponente. Sie empfängt Ereignisse aus dem Funknetz, wertet sie aus, fragt bei Bedarf das Directory ab und verteilt daraus entstehende Informationen an Dashboard oder Funkgeräte zurück.</p><hr><h2>Hauptkomponenten</h2><h3>Basisstation</h3><p>Die Basisstation ist der technische Kern des Systems.</p><p>Sie übernimmt unter anderem:</p><ul><li><p>Aussendung der TETRA-Zelle,</p></li><li><p>Verarbeitung von Registrierungen,</p></li><li><p>Verwaltung aktiver Teilnehmer,</p></li><li><p>Gruppenanbindungen,</p></li><li><p>Gruppenrufe,</p></li><li><p>Einzelrufe,</p></li><li><p>SDS-Nachrichten,</p></li><li><p>Statusmeldungen,</p></li><li><p>GPS-/LIP-Daten,</p></li><li><p>Rückmeldungen an Funkgeräte,</p></li><li><p>Weitergabe von Ereignissen an Dashboard und Gateways.</p></li></ul><p>Die Basisstation arbeitet zustandsorientiert. Sie merkt sich registrierte Geräte, Gruppenbeziehungen, aktive Rufe, aktuelle Statusinformationen und temporäre SDS-Zustände.</p><hr><h3>Dashboard</h3><p>Das Dashboard ist die Bedien- und Beobachtungsoberfläche der Basisstation.</p><p>Es zeigt:</p><ul><li><p>registrierte Funkgeräte,</p></li><li><p>Online- und Ruhezustände,</p></li><li><p>SDS-Nachrichten,</p></li><li><p>Statusmeldungen,</p></li><li><p>GPS-/Positionsdaten,</p></li><li><p>laufende oder vergangene Rufe,</p></li><li><p>Systemzustand,</p></li><li><p>Gateway- und Verbindungsstatus.</p></li></ul><p>Das Dashboard erhält seine Daten überwiegend über Telemetrie- und WebSocket-Ereignisse der Basisstation. Es ist damit eine Live-Ansicht des aktuellen Netzbetriebs.</p><hr><h3>NetCore Directory</h3><p>Das NetCore Directory ist die Stammdatenverwaltung.</p><p>Es speichert:</p><ul><li><p>Geräte,</p></li><li><p>Basisstationen,</p></li><li><p>GSSI-Gruppen,</p></li><li><p>Statusmeldungen,</p></li><li><p>Gerätegruppen,</p></li><li><p>Fahrzeug-/Statusgruppen.</p></li></ul><p>Die Basisstation nutzt das Directory, um technische IDs in verständliche Informationen umzuwandeln.</p><p>Beispiele:</p><pre><code class="language-text">ISSI 2020001 → HRT Fahrer
ISSI 2020002 → MRT Fahrzeug
GSSI 15201   → Betriebsgruppe
Status 1     → Frei auf Funk
</code></pre><p>Das Directory ist bewusst getrennt von der Basisstation. Dadurch können Stammdaten geändert werden, ohne die eigentliche Funklogik in der Basisstation fest zu verdrahten.</p><hr><h3>Funkgeräte</h3><p>Funkgeräte sind die Teilnehmer im Netz. Sie können sich registrieren, Gruppen anhängen, Sprache übertragen, SDS senden, Statusmeldungen absetzen und je nach Gerätetyp Positionsdaten liefern.</p><p>Typische Rollen:</p><ul><li><p>HRT,</p></li><li><p>MRT,</p></li><li><p>Gateway-Gerät,</p></li><li><p>Bediengerät,</p></li><li><p>Testgerät,</p></li><li><p>Infrastrukturteilnehmer.</p></li></ul><p>Für die Basisstation ist zunächst die ISSI entscheidend. Weitere Bedeutung entsteht erst durch Directory-Daten, Rollen, Gruppen und Statuslogik.</p><hr><h3>Optionale Gateways</h3><p>NetCore-Tetra kann weitere Dienste anbinden.</p><p>Beispiele:</p><ul><li><p>SIP-/Telefonie-Gateway,</p></li><li><p>externe SDS-Dienste,</p></li><li><p>Alarm- oder Statusweiterleitungen,</p></li><li><p>Karten- und Positionsdienste,</p></li><li><p>Betriebs- oder Monitoringdienste.</p></li></ul><p>Diese Schnittstellen sind optional. Das Grundsystem aus Basisstation, Dashboard und Directory funktioniert auch ohne externe Dienste.</p><hr><h2>Datenflüsse</h2><h3>Registrierung eines Geräts</h3><p>Wenn ein Funkgerät die Zelle findet und sich registriert, entsteht ein Mobility-Management-Ereignis.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät
   │
   │ Location Update / Attach
   ▼
Basisstation
   │
   ├── Registrierung prüfen
   ├── Teilnehmerzustand aktualisieren
   ├── Dashboard informieren
   └── ggf. gespeicherten Status erneut zustellen
</code></pre><p>Bei erfolgreicher Registrierung kennt die Basisstation das Gerät als aktiven Teilnehmer. Falls für dieses Gerät bereits ein Status gespeichert ist, kann dieser erneut per Display-Rückmeldung zugestellt werden.</p><hr><h3>Gruppenanbindung</h3><p>Ein Gerät kann sich einer GSSI-Gruppe anschließen.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät
   │
   │ Gruppenanbindung / Affiliation
   ▼
Basisstation
   │
   ├── Gruppenzuordnung speichern
   ├── Dashboard aktualisieren
   └── Gruppenrufe entsprechend routen
</code></pre><p>Die GSSI selbst ist technisch nur eine Nummer. Der lesbare Name kommt aus dem Directory.</p><hr><h3>Statusmeldung</h3><p>Statusmeldungen sind ein zentraler Bestandteil von NetCore-Tetra.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät
   │
   │ U-STATUS
   ▼
Basisstation
   │
   ├── Statuscode lesen
   ├── Statuslabel im Directory suchen
   ├── Dashboard aktualisieren
   ├── Statuscache aktualisieren
   ├── ggf. Statusgruppe ermitteln
   └── Display-Rückmeldung an Geräte senden
</code></pre><p>Beispiel:</p><pre><code class="language-text">2020001 sendet Status 1
Directory: Status 1 = Frei auf Funk
Dashboard: 2020001 → Frei auf Funk
Funkgerät: Status: Frei auf Funk
</code></pre><p>Wenn das Gerät Mitglied einer Statusgruppe ist, wird der Status nicht nur für das sendende Gerät übernommen, sondern für alle Mitglieder dieser Gruppe.</p><hr><h3>Statusgruppe / Fahrzeuglogik</h3><p>Mehrere Geräte können logisch zu einer gemeinsamen Einheit gehören. Diese Einheit kann zum Beispiel ein Fahrzeug, ein Trupp oder eine taktische Gruppe sein.</p><p>Beispiel:</p><pre><code class="language-text">Statusgruppe: RTW 83-01

Mitglieder:
- 2020001 HRT Fahrer
- 2020002 MRT Fahrzeug
- 2020003 HRT Beifahrer
</code></pre><p>Sendet eines dieser Geräte einen Status, übernimmt die gesamte Gruppe diesen Status.</p><p>Ablauf:</p><pre><code class="language-text">2020001 sendet Status
   │
   ▼
Basisstation fragt Directory:
Welche Statusgruppe enthält 2020001?
   │
   ▼
Directory antwortet:
2020001, 2020002, 2020003
   │
   ▼
Basisstation setzt Status für alle Mitglieder
   │
   ├── Dashboard zeigt alle Mitglieder gleich
   ├── angemeldete Geräte erhalten Display-Rückmeldung
   └── offline Geräte erhalten den Status beim nächsten Join
</code></pre><p>Dadurch entsteht eine fahrzeugbezogene Statuslogik, ohne dass jedes einzelne Gerät separat denselben Status senden muss.</p><hr><h3>Live Directory Sync</h3><p>Die Basisstation kann Directory-Gruppenzuordnungen regelmäßig aktualisieren.</p><p>Dadurch werden Änderungen an Gerätegruppen nahezu live übernommen.</p><p>Beispiel:</p><pre><code class="language-text">1. Statusgruppe enthält 2020001 und 2020002
2. 2020001 sendet Status „Frei auf Funk“
3. Beide Geräte zeigen den Status
4. Im Directory werden 2020003 und 2020004 ergänzt
5. Basisstation zieht die Gruppe neu
6. 2020003 und 2020004 übernehmen den vorhandenen Gruppenstatus
</code></pre><p>Die Basisstation muss dafür keinen neuen Status abwarten. Sie erkennt die erweiterte Gruppe durch regelmäßige Aktualisierung der Directory-Daten.</p><hr><h3>SDS</h3><p>SDS-Nachrichten werden für kurze Datenübertragungen genutzt.</p><p>Mögliche SDS-Arten im System:</p><ul><li><p>Textnachrichten,</p></li><li><p>Steuerbefehle,</p></li><li><p>Statusrückmeldungen,</p></li><li><p>Display-Nachrichten,</p></li><li><p>Positions- oder Sonderdaten,</p></li><li><p>interne Dashboard-/Logdarstellung.</p></li></ul><p>Ablauf einer SDS-Nachricht:</p><pre><code class="language-text">Quelle
   │
   │ SDS
   ▼
Basisstation
   │
   ├── Ziel prüfen
   ├── Typ auswerten
   ├── ggf. Text dekodieren
   ├── Dashboard loggen
   └── Nachricht zustellen oder verarbeiten
</code></pre><p>Nicht jede SDS wird unverändert weitergeleitet. Manche SDS werden lokal ausgewertet, zum Beispiel Statusrückmeldungen oder Steuerbefehle.</p><hr><h3>Home Mode Display</h3><p>Für lesbare Rückmeldungen an Funkgeräte kann die Basisstation eine Display-Nachricht senden.</p><p>Beispiel:</p><pre><code class="language-text">Status: Frei auf Funk
</code></pre><p>Diese Rückmeldung ist besonders bei Statusmeldungen nützlich. Das Funkgerät sendet nur eine Statusnummer, erhält aber anschließend den lesbaren Text zurück.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät sendet Statusnummer
   │
   ▼
Basisstation löst Text im Directory auf
   │
   ▼
Basisstation sendet Display-Text an Funkgerät
</code></pre><p>Bei Statusgruppen kann dieselbe Rückmeldung an mehrere Geräte gesendet werden.</p><hr><h3>GPS / LIP</h3><p>Wenn ein Gerät Positionsdaten übermittelt, kann die Basisstation diese auswerten und an das Dashboard weitergeben.</p><p>Ablauf:</p><pre><code class="language-text">Funkgerät
   │
   │ LIP / Positionsdaten
   ▼
Basisstation
   │
   ├── Position dekodieren
   ├── ISSI zuordnen
   ├── Directory-Name ergänzen
   └── Dashboard-Karte aktualisieren
</code></pre><p>Das Dashboard kann daraus Gerätepositionen darstellen. Sichtbarkeit und Bezeichnung ergeben sich aus der Kombination von Funkdaten und Directory-Stammdaten.</p><hr><h2>Zustände in der Basisstation</h2><p>Die Basisstation verwaltet mehrere Arten von Zustand.</p><h3>Teilnehmerzustand</h3><p>Dazu gehören:</p><ul><li><p>registriert,</p></li><li><p>nicht registriert,</p></li><li><p>Online,</p></li><li><p>Schlaf-/Ruhezustand,</p></li><li><p>letzte RSSI-Information,</p></li><li><p>Gruppenanbindung,</p></li><li><p>aktive oder vergangene Rufe.</p></li></ul><h3>Statuszustand</h3><p>Dazu gehören:</p><ul><li><p>letzter Status pro ISSI,</p></li><li><p>letzter Status pro Statusgruppe,</p></li><li><p>zuletzt gesendete Display-Rückmeldung,</p></li><li><p>Replay-Information für Rejoin,</p></li><li><p>Throttle-Zeitpunkte gegen Antwort-Spam.</p></li></ul><h3>SDS-Zustand</h3><p>Dazu gehören:</p><ul><li><p>ausstehende SDS-Zustellungen,</p></li><li><p>zurückgestellte SDS während eines Rufs,</p></li><li><p>Delivery Reports,</p></li><li><p>interne SDS-Logs,</p></li><li><p>Steuer-SDS.</p></li></ul><h3>Directory-Zustand</h3><p>Dazu gehören gecachte Informationen wie:</p><ul><li><p>Statuscodes,</p></li><li><p>Statusgruppenmitglieder,</p></li><li><p>Gerätebezeichnungen,</p></li><li><p>Gruppenbezeichnungen,</p></li><li><p>Ablaufzeiten für erneute Directory-Abfragen.</p></li></ul><hr><h2>Cache-Strategie</h2><p>Nicht jede Information wird bei jeder Aktion neu geladen. Das wäre unnötig langsam und würde das Directory stark belasten.</p><p>NetCore-Tetra nutzt daher Cache-Mechanismen.</p><p>Typische Cache-Bereiche:</p>
Bereich | Zweck
-- | --
Statusmeldungen | Statuscode schnell in Text auflösen
Statusgruppen | Mitglieder einer Statusgruppe ermitteln
Geräteinformationen | ISSI im Dashboard lesbar anzeigen
Rejoin-Status | Status nach erneuter Registrierung wieder zustellen

<hr><h2>Designprinzipien</h2><p>NetCore-Tetra folgt mehreren Grundsätzen:</p><h3>Lokal betreibbar</h3><p>Das System soll in einem lokalen Netz funktionieren. Internetzugriff darf nicht zwingend notwendig sein.</p><h3>Betreiberhoheit</h3><p>Der Betreiber soll Kontrolle über Geräte, Gruppen, Status, IDs und Schnittstellen behalten.</p><h3>Verständliche Daten</h3><p>Technische IDs sollen möglichst früh in lesbare Namen übersetzt werden.</p><h3>Modularität</h3><p>Funklogik, Directory, Dashboard und optionale Gateways sollen getrennt bleiben.</p><h3>Robuste Fallbacks</h3><p>Ein Ausfall einzelner Zusatzkomponenten darf nicht automatisch den Funkbetrieb stoppen.</p><h3>Erweiterbarkeit</h3><p>Neue Dienste, Statuslogiken, Gerätegruppen oder Dashboard-Ansichten sollen später ergänzt werden können.</p><hr><h2>Nächste Seiten</h2><p>Zum weiteren Verständnis dieser Architektur sind besonders relevant:</p><ul><li><p>[[Installation]]</p></li><li><p>[[Configuration]]</p></li><li><p>[[NetCore-Directory]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>