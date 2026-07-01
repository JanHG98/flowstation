# Operator-Profile

Der native Operator kann lokale Profile lesen. Dadurch müssen API-URL, Token, Standard-Node und Operator-ID nicht bei jedem Befehl angegeben werden.

## Suchreihenfolge der Config

1. `--config /pfad/operator.toml`
2. `NETCORE_CONTROL_ROOM_OPERATOR_CONFIG`
3. `$XDG_CONFIG_HOME/netcore/control-room/operator.toml`
4. `$HOME/.config/netcore/control-room/operator.toml`
5. `/etc/netcore-control-room/operator.toml`

## Suchreihenfolge der Werte

API:

1. `--api`
2. `NETCORE_CONTROL_ROOM_API`
3. Profilwert `api`
4. `http://127.0.0.1:9010`

Token:

1. `--token`
2. `--token-file`
3. `NETCORE_CONTROL_ROOM_TOKEN`
4. `NETCORE_CONTROL_ROOM_OPERATOR_TOKEN`
5. Profilwert `token`
6. Profilwert `token_file`

Default Node:

1. `NETCORE_CONTROL_ROOM_NODE_ID`
2. Profilwert `default_node`
3. `tbs-04010001`

Operator-ID:

1. `NETCORE_CONTROL_ROOM_OPERATOR_ID`
2. Profilwert `operator_id`
3. `operator`

## Beispiel

```toml
[profiles.default]
api = "http://10.0.1.25:9010"
default_node = "SRV-M_TBS-01"
operator_id = "jan"
token_file = "/etc/netcore-control-room/operator.token"
```

## Prüfen

```bash
netcore-control-room-operator profiles show
```

Die Ausgabe zeigt nicht den Klartext-Token, sondern nur `token_present` und `token_source`.
