# Native Operator UI

Die vorhandene native Windows-UI bleibt als Audio- und Operator-Client erhalten. Ihr Login-Dialog stammt aus dem vorbereiteten späteren RBAC-Betrieb.

## Aktuelle Open-Lab-Phase

Der Control-Room-Server wird mit `--no-auth` betrieben. Deshalb sind weder Passwort noch Token serverseitig erforderlich. Für die aktuelle Referenzverwaltung ist die neue Browser-WebUI auf Port `9010` maßgeblich.

Ein lokales Profil darf weiterhin Komfortwerte enthalten:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
username = "jan"

[ui.map]
online_tiles = true
default_lat = 52.3759
default_lon = 9.7320
default_zoom = 13
```

Der native Client muss vor einem produktiven Einsatz an den späteren gesicherten Authentisierungsmodus angepasst und gemeinsam mit TLS/RBAC getestet werden. Ein in der Oberfläche abgefragtes Passwort erzeugt im Open-Lab-Modus keine zusätzliche Sicherheit.
