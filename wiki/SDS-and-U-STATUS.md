# SDS und U-STATUS

## SDS

Die Basisstation verarbeitet Text- und Datennachrichten zwischen lokalen Teilnehmern sowie optional über angebundene Dienste. Eingänge und Ausgänge erscheinen im SDS-Log des Dashboards.

Textdaten können verschiedene Protokollkennungen und Zeichensätze verwenden. Bei gemischten Gerätegenerationen sollte mit realen Umlauten, Sonderzeichen und maximaler Textlänge getestet werden.

## U-STATUS

U-STATUS überträgt einen numerischen Statuswert. Die Basisstation kann:

- den Status im Dashboard anzeigen,
- Directory-Labels zuordnen,
- Home-Mode-Display beantworten,
- Statusgruppen synchronisieren,
- Notfalllogik auslösen,
- definierte Systembefehle ausführen.

## Systembefehle

In `[cell_info.sds_command_control]` werden autorisierte ISSIs und die Zuordnung von Statuscodes zu Aktionen festgelegt. Die aktuelle Steueradresse der Basisstation ist die System-ISSI `4010001`. Unterstützte Aktionen umfassen:

- Dienst neu starten
- System herunterfahren
- registrierte Geräte abmelden
- IP-Adresse abfragen
- Temperatur abfragen
- Systeminformationen abfragen

Beispielstruktur:

```toml
[cell_info.sds_command_control]
authorized_issis = [<AUTORISIERTE-ISSI>]

[[cell_info.sds_command_control.commands]]
status_code = <STATUSCODE>
action = "info"
```

Die Statuscodes sind frei planbar, müssen aber zur Programmierung der Endgeräte passen. Die Sektion wird durch ihre Anwesenheit aktiviert; ein zusätzliches `enabled`-Feld existiert hier nicht. Keine breit gefasste Autorisierung verwenden.

## Sicherheit

- Befehlsquelle auf bekannte ISSIs beschränken.
- Steuer-ISSI nicht als normales Benutzergerät verwenden.
- Shutdown/Restart nur in einem kontrollierten Labornetz aktivieren.
- Eingehende Befehle im SDS-Log und Journal protokollieren.
- Eine ISSI ist keine kryptografische Identität; RF- und Netzzugang bleiben die eigentliche Schutzgrenze.
