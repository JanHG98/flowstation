<html><body>
<!--StartFragment--><html><head></head><body><h1>Systemd-Service</h1><p>Diese Seite beschreibt den Betrieb von NetCore-Tetra über <code>systemd</code>.</p><p>Mit systemd können Basisstation und NetCore Directory automatisch gestartet, überwacht, neu gestartet und sauber geloggt werden. Dadurch läuft das System nicht nur als manuell gestarteter Prozess, sondern als richtiger Dienst.</p><hr><h2>Überblick</h2><p>Eine typische NetCore-Tetra-Installation nutzt mindestens zwei Dienste:</p><pre><code class="language-text">netcore-directory.service
tetra.service
</code></pre><p>Empfohlene Rollen:</p>
Dienst | Aufgabe
-- | --
netcore-directory.service | startet das NetCore Directory
tetra.service | startet die NetCore-Tetra Basisstation
optional weitere Dienste | Gateways, Monitoring, Backup, Control UI

<p>Wichtig ist Konsistenz. Wenn <code>service_name</code> in der Config genutzt wird, sollte er zur Unit passen.</p><hr><h2>Healthcheck über systemd</h2><p>Ein einfacher Dienststatus:</p><pre><code class="language-bash">systemctl is-active tetra.service
</code></pre><p>Directory:</p><pre><code class="language-bash">systemctl is-active netcore-directory.service
</code></pre><p>Kombiniert:</p><pre><code class="language-bash">systemctl is-active tetra.service &amp;&amp; \
systemctl is-active netcore-directory.service
</code></pre><hr><h2>Boot-Test</h2><p>Nach Einrichtung sollte ein vollständiger Boot getestet werden.</p><p>Ablauf:</p><pre><code class="language-bash">sudo reboot
</code></pre><p>Nach dem Neustart:</p><pre><code class="language-bash">systemctl status netcore-directory.service
systemctl status tetra.service
</code></pre><p>Logs seit Boot:</p><pre><code class="language-bash">sudo journalctl -u tetra.service -b
sudo journalctl -u netcore-directory.service -b
</code></pre><hr><h2>Empfohlene Reihenfolge nach Änderungen</h2><p>Bei Änderungen an systemd-Units:</p><pre><code class="language-bash">sudo systemctl daemon-reload
sudo systemctl restart netcore-directory.service
sudo systemctl restart tetra.service
sudo journalctl -u tetra.service -f
</code></pre><p>Bei Änderungen nur an der Basisstations-Config:</p><pre><code class="language-bash">sudo systemctl restart tetra.service
</code></pre><p>Bei Änderungen nur am Directory-Code:</p><pre><code class="language-bash">sudo systemctl restart netcore-directory.service
</code></pre><hr><h2>Nächste Seiten</h2><p>Weiterführende Seiten:</p><ul><li><p>[[Configuration]]</p></li><li><p>[[NetCore-Directory]]</p></li><li><p>[[Status-Feedback]]</p></li><li><p>[[Status-Groups]]</p></li><li><p>[[Troubleshooting]]</p></li></ul></body></html><!--EndFragment-->
</body>
</html>