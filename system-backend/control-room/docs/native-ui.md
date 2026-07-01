# Native UI – Login ab v5.0

Die native Windows-UI nutzt keinen Operator-Token mehr. Sie zeigt eine Loginmaske.

Windows-Config:

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

Passwort wird beim Start in der UI eingegeben.
