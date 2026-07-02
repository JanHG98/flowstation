<html><body>
<!--StartFragment--><html><head></head><body><h1>Configuration</h1><p>Diese Seite beschreibt die zentrale Konfiguration von NetCore-Tetra.</p><p>Die Konfiguration erfolgt über eine TOML-Datei, typischerweise:</p><pre><code class="language-text">/opt/netcore-tetra/config/config.toml
</code></pre><p>oder im Entwicklungsbetrieb direkt im Projektverzeichnis:</p><pre><code class="language-text">./config.toml
</code></pre><p>Die <code>config.toml</code> steuert unter anderem:</p><ul><li><p>Betriebsmodus,</p></li><li><p>SDR-Hardware,</p></li><li><p>Frequenzen,</p></li><li><p>Zellparameter,</p></li><li><p>Netzkennung,</p></li><li><p>Dashboard,</p></li><li><p>NetCore Directory,</p></li><li><p>Statuslogik,</p></li><li><p>SDS-Funktionen,</p></li><li><p>Telefonie,</p></li><li><p>Sicherheit,</p></li><li><p>Wiederherstellung und Monitoring.</p></li></ul><blockquote><p><strong>Hinweis:</strong> Änderungen an der Konfiguration können direkten Einfluss auf den Funkbetrieb haben. Frequenzen, Netzparameter und Sendeleistung müssen vor dem Start geprüft werden.</p></blockquote><hr><h2>Grundaufbau</h2><p>Eine typische Konfiguration besteht aus mehreren Abschnitten:</p><pre><code class="language-toml">config_version = "0.6"
stack_mode = "Bs"

[phy_io]
# SDR / RF-Hardware

[net_info]
# Netzkennung

[cell_info]
# Zellparameter

[dashboard]
# Web-Dashboard

[netcore_directory]
# NetCore Directory

[asterisk]
# optionale Telefonie

[security]
# Zugriffsschutz

[recovery]
# Wiederherstellung
</code></pre><p>Jeder Abschnitt beschreibt einen Funktionsbereich. Nicht jede Funktion muss aktiv sein.</p><hr><h2>Konfigurationsversion</h2><p>Die Konfigurationsversion steht am Anfang der Datei:</p><pre><code class="language-toml">config_version = "0.6"
</code></pre><p>Diese Version beschreibt das erwartete Format der Konfigurationsdatei.</p><p>Wenn die Version nicht zur Software passt, kann der Start abgebrochen werden.</p><hr><h2>Stack-Modus</h2><p>Der Stack-Modus legt fest, in welcher Betriebsart NetCore-Tetra startet.</p><pre><code class="language-toml">stack_mode = "Bs"
</code></pre><p>Mögliche Betriebsarten:</p>
Wert | Bedeutung
-- | --
Bs | Basisstation
Ms | Mobilstation
Mon | Monitorbetrieb

<hr><h2>Rufnummernlogik</h2><p>Die Rufnummernlogik wird über Präfixe gesteuert.</p><p>Beispiel:</p><pre><code class="language-toml">outbound_prefix = "91"
strip_outbound_prefix = true
inbound_prefix = "T"
</code></pre><p>Ablauf Funkgerät zu Telefon:</p><pre><code class="language-text">Funkgerät wählt 91102
→ Präfix 91 wird erkannt
→ Ziel 102 wird an Telefonie übergeben
</code></pre><p>Ablauf Telefon zu Funkgerät:</p><pre><code class="language-text">Telefon wählt T2020001
→ Ziel-ISSI 2020001 wird gerufen
</code></pre><p>Je nach Konfiguration können Kurzformen, Service-Nummern oder Präfixe angepasst werden.</p><hr><h2>Service Numbers</h2><p>Service Numbers begrenzen, welche Nummern aus dem Funknetz heraus erreichbar sind.</p><p>Beispiel:</p><pre><code class="language-toml">service_numbers = ["102", "103"]
</code></pre><p>Zum Testen kann breiter erlaubt werden:</p><pre><code class="language-toml">service_numbers = ["*"]
</code></pre><p>Für produktiveren Betrieb ist eine enge Liste sinnvoll.</p><hr><h2>SDS Command Control</h2><p>SDS Command Control ermöglicht Steuerbefehle per Status oder SDS.</p><p>Beispiele für mögliche Funktionen:</p><pre><code class="language-text">Restart Service
Shutdown Service
Kick Teilnehmer
Clear Emergency
</code></pre><p>Solche Funktionen sollten nur bewusst aktiviert werden.</p><p>Empfehlungen:</p><pre><code class="language-text">- nur bekannte ISSIs zulassen
- eindeutige Statuscodes nutzen
- keine kritischen Befehle ungeschützt aktivieren
- Logs beobachten
</code></pre><hr><h2>Statusmeldungen</h2><p>Statusmeldungen werden technisch als Statuscode empfangen und über das Directory in lesbare Texte umgewandelt.</p><p>Beispiel:</p><pre><code class="language-text">Statuscode 1 → Frei auf Funk
Statuscode 2 → Einsatzbereit
Statuscode 6 → Nicht einsatzbereit
</code></pre><p>Die Statuslogik nutzt:</p><pre><code class="language-text">- Statuscode aus dem Funkgerät
- Directory-Statuslabel
- Dashboard-Anzeige
- HMD-Rückmeldung
- Statusgruppen
- Replay bei Rejoin
</code></pre><p>Die eigentlichen Statuslabels werden im NetCore Directory gepflegt.</p><hr><h2>Statusgruppen</h2><p>Statusgruppen werden nicht direkt in der <code>config.toml</code>, sondern im Directory gepflegt.</p><p>Die Basisstation benötigt dafür nur:</p><pre><code class="language-toml">[netcore_directory]
enabled = true
base_url = "http://127.0.0.1:8095"
timeout_ms = 2000
</code></pre><p>Ablauf:</p><pre><code class="language-text">Funkgerät sendet Status
→ Basisstation fragt Directory nach Statusgruppe
→ Status wird auf alle Gruppenmitglieder angewendet
</code></pre><hr><h2>Security</h2><p>Sicherheitsfunktionen werden im Abschnitt <code>[security]</code> konfiguriert.</p><p>Typische Themen:</p><pre><code class="language-text">- erlaubte ISSIs
- Dashboard-Zugriff
- Steuerbefehle
- externe Gateways
- Zugriff auf Update-Funktionen
</code></pre><p>Für lokale Testnetze kann vieles offen sein. Für dauerhaften Betrieb sollte der Zugriff bewusst eingeschränkt werden.</p><hr><h2>Dashboard-Zugriff absichern</h2><p>Wenn das Dashboard im LAN erreichbar ist, sollte mindestens ein Login gesetzt werden.</p><p>Beispiel:</p><pre><code class="language-toml">[dashboard]
enabled = true
bind_addr = "0.0.0.0"
port = 8080
username = "admin"
password = "change-me"
</code></pre><p>Empfehlungen:</p><pre><code class="language-text">- kein Standardpasswort verwenden
- Dashboard nicht unnötig ins Internet stellen
- Zugriff per Firewall oder VPN begrenzen
- Updates und Neustarts nur autorisiert zulassen
</code></pre><hr><h2>Recovery</h2><p>Recovery-Funktionen dienen dazu, nach Neustarts oder Fehlern schneller wieder einen stabilen Zustand zu erreichen.</p><p>Beispielbereiche:</p><pre><code class="language-text">- Wiederregistrierung
- Fallback-Konfiguration
- Neustart nach Fehlern
- Status-Replay
</code></pre><p>Die genaue Nutzung hängt vom aktuellen Softwarestand ab.</p><hr><h2>Fallback-Konfiguration</h2><p>Eine Fallback-Datei ist dringend empfehlenswert.</p><p>Beispiel:</p><pre><code class="language-bash">cp config.toml config.toml.fallback
</code></pre><p>Typisches Ziel:</p><pre><code class="language-text">config.toml.fallback = letzter bekannter funktionierender Stand
</code></pre><p>Wenn die Hauptkonfiguration fehlerhaft ist, kann das System auf den Fallback zurückgreifen oder zumindest leichter wiederhergestellt werden.</p><hr><h2>Telegram / externe Benachrichtigungen</h2><p>Optionale Benachrichtigungen können in eigenen Abschnitten konfiguriert werden.</p><p>Typische Einsatzzwecke:</p><pre><code class="language-text">- Emergency-Hinweise
- Systemzustand
- Fehleralarme
- Statusereignisse
</code></pre><p>Diese Funktionen sind optional und sollten nur eingerichtet werden, wenn die Zugangsdaten und Zielkanäle sicher verwaltet werden.</p><hr><h2>Geoalarm</h2><p>Geoalarm-Funktionen können Positionsdaten auswerten und bei bestimmten Bedingungen reagieren.</p><p>Mögliche Logik:</p><pre><code class="language-text">- Position empfangen
- Zone prüfen
- Alarm auslösen
- Dashboard markieren
- Nachricht weiterleiten
</code></pre><p>Dafür müssen GPS-/LIP-Daten verfügbar sein.</p><hr><h2>WX / Wetterdienst</h2><p>Ein optionaler Wetterdienst kann Wetterinformationen per SDS bereitstellen.</p><p>Typische Funktionen:</p><pre><code class="language-text">- Wetterabfrage per SDS
- periodische Wetter-SDS
- METAR-Auswertung
- lokale Konfiguration von Stationen
</code></pre><p>Diese Funktion ist optional und nicht für den Grundbetrieb erforderlich.</p><hr><h2>Beispiel: Minimaler lokaler Betrieb</h2><p>Eine stark vereinfachte Beispielkonfiguration:</p><pre><code class="language-toml">config_version = "0.6"
stack_mode = "Bs"

[phy_io]
backend = "SoapySdr"

[phy_io.soapysdr]
tx_freq = 418000000
rx_freq = 408000000

[net_info]
mcc = 901
mnc = 1510

[cell_info]
freq_band = 4
main_carrier = 720

[dashboard]
enabled = true
bind_addr = "0.0.0.0"
port = 8080

[netcore_directory]
enabled = true
base_url = "http://127.0.0.1:8095"
timeout_ms = 2000
</code></pre><p>Diese Beispielkonfiguration ist nicht vollständig und muss an Hardware, Frequenzen und Betriebsumgebung angepasst werden.</p><hr><h2>Beispiel: Directory auf separatem Host</h2><pre><code class="language-toml">[netcore_directory]
enabled = true
base_url = "http://10.0.1.22:8095"
timeout_ms = 2000
</code></pre><p>Prüfen von der Basisstation aus:</p><pre><code class="language-bash">curl -s http://10.0.1.22:8095/api/health | jq .
</code></pre><hr><h2>Beispiel: Statusgruppen-Test</h2><p>Voraussetzungen:</p><pre><code class="language-text">- Directory läuft
- Geräte sind im Directory eingetragen
- Gerätegruppe ist im Directory angelegt
- status_sync ist aktiv
- Mitglieder sind korrekt gesetzt
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
</code></pre><p>Erwartung:</p><pre><code class="language-json">{
  "issi": 2020001,
  "status_sync_members": [
    2020001,
    2020002
  ]
}
</code></pre><hr><h2>Prüfung nach Änderungen</h2><p>Nach Änderungen an der Konfiguration:</p><pre><code class="language-bash">cargo build --release --features asterisk
sudo systemctl restart tetra.service
sudo journalctl -u tetra.service -f
</code></pre><p>Wenn nur die Config geändert wurde, reicht meist:</p><pre><code class="language-bash">sudo systemctl restart tetra.service
</code></pre><hr><h2>Häufige Konfigurationsfehler</h2><h3>Falsche Frequenzen</h3><p>Symptome:</p><pre><code class="language-text">- Funkgerät sieht keine Zelle
- Registrierung schlägt fehl
- Uplink kommt nicht an
- Gerät zeigt Netz, kann aber nicht arbeiten
</code></pre><p>Prüfen:</p><pre><code class="language-text">- tx_freq
- rx_freq
- freq_band
- main_carrier
- Codeplug des Funkgeräts
- Duplexabstand
</code></pre><hr><h3>Directory deaktiviert</h3><p>Symptome:</p><pre><code class="language-text">- Dashboard zeigt nur ISSIs
- Status wird nicht als Text angezeigt
- Statusgruppen greifen nicht
</code></pre><p>Prüfen:</p><pre><code class="language-toml">[netcore_directory]
enabled = true
</code></pre><p>und:</p><pre><code class="language-bash">curl -s http://127.0.0.1:8095/api/health | jq .
</code></pre><hr><h3>Falsche Directory-URL</h3><p>Symptome:</p><pre><code class="language-text">- Directory funktioniert im Browser
- Basisstation bekommt aber keine Namen oder Statusgruppen
</code></pre><p>Prüfen:</p><pre><code class="language-bash">curl -s http://&lt;DIRECTORY-IP&gt;:8095/api/health | jq .
</code></pre><p>Die Prüfung muss von der Basisstation aus funktionieren.</p><hr><h3>Dashboard nicht erreichbar</h3><p>Prüfen:</p><pre><code class="language-toml">[dashboard]
enabled = true
bind_addr = "0.0.0.0"
port = 8080
</code></pre><p>und:</p><pre><code class="language-bash">sudo ss -tulpn | grep 8080
</code></pre><hr><h3>Telefonie funktioniert nicht</h3><p>Prüfen:</p><pre><code class="language-text">- asterisk.enabled
- remote_host
- remote_port
- local_user
- auth_user
- password
- service_numbers
- RTP-Portbereich
- Firewall
</code></pre><p>Logs:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -f | egrep "Asterisk|SIP|RTP|service_numbers"
</code></pre><hr><h2>Empfohlene Arbeitsweise</h2><p>Bei Änderungen:</p><pre><code class="language-text">1. Config sichern
2. Änderung vornehmen
3. Dienst neu starten
4. Logs beobachten
5. Funktion testen
6. funktionierenden Stand als Fallback sichern
</code></pre><p>Backup:</p><pre><code class="language-bash">cp config.toml config.toml.$(date +%F-%H%M).bak
</code></pre><p>Fallback aktualisieren, wenn alles stabil läuft:</p><pre><code class="language-bash">cp config.toml config.toml.fallback
</code></pre><hr><h2>Nächste Seiten</h2><p>Passende weiterführende Seiten:</p><ul><li><p>[[Systemd-Service]]</p></li><li><p>[[NetCore-Directory]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Dashboard]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>