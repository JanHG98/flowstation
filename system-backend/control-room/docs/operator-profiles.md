# Operator Profiles ab v5.0

Operator-Profile speichern nur lokale Komfortwerte. Keine Tokens und keine Passwörter.

Beispiel:

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
username = "jan"
```

Die Windows-UI fragt das Passwort beim Start ab.

Die CLI kann Passwort optional per Env oder Datei bekommen:

```bash
export NETCORE_CONTROL_ROOM_USER=jan
export NETCORE_CONTROL_ROOM_PASSWORD='<passwort>'
./target/release/netcore-control-room-operator overview
```
