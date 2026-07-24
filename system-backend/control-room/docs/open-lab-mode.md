# Open-Lab-Modus

Der aktuelle Control Room läuft bewusst ohne Login, Token und TLS. Auch der TBS-WebSocket `/node` verlangt in dieser Stufe keinen Maschinen-Token.

Das ist keine Sicherheitsfunktion und kein geeignetes Produktivprofil. Netzwerkseitig erforderlich sind mindestens:

- eigenes Management-VLAN oder vollständig isoliertes Labornetz,
- kein Port-Forwarding aus dem Internet,
- Firewall-Regeln nur für Testarbeitsplätze und die bekannten LXCs,
- keine realen Produktionsschlüssel oder personenbezogenen Echtdaten.

Die vorhandene Auth-/RBAC-Implementierung bleibt als später aktivierbare Grundlage im Code, wird aber im Beispiel und im systemd-Service ausdrücklich mit `--no-auth` deaktiviert.
