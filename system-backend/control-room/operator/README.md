# NetCore Control Room Operator

Native Operator-Konsole für den NetCore Control Room. Keine Web-App.

## Start

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token "$NETCORE_CONTROL_ROOM_OPERATOR_TOKEN" \
  dashboard
```

Alternativ:

```bash
export NETCORE_CONTROL_ROOM_API=http://10.0.1.25:9010
export NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=<token>

./target/release/netcore-control-room-operator dashboard
```

## Ansichten

```bash
netcore-control-room-operator overview
netcore-control-room-operator subscribers --online
netcore-control-room-operator groups
netcore-control-room-operator calls
netcore-control-room-operator locations
netcore-control-room-operator sds --limit 20
netcore-control-room-operator commands --limit 20
```

## Commands

```bash
netcore-control-room-operator kick --node SRV-M_TBS-01 --issi 2010002 --operator jan
netcore-control-room-operator dgna --node SRV-M_TBS-01 --issi 2020004 --gssi 15205 --operator jan
netcore-control-room-operator dgna --node SRV-M_TBS-01 --issi 2020004 --gssi 15205 --detach --operator jan
netcore-control-room-operator clear-emergency --node SRV-M_TBS-01 --operator jan
```

## Tokenverwaltung / RBAC

Tokenliste:

```bash
netcore-control-room-operator tokens list
```

Token erstellen, Klartext wird nur einmal angezeigt:

```bash
netcore-control-room-operator tokens create --label "ELW Display" --role viewer --created-by jan
netcore-control-room-operator tokens create --label "Jan Operator" --role operator --created-by jan
netcore-control-room-operator tokens create --label "Jan Admin" --role admin --created-by jan
netcore-control-room-operator tokens create --label "TBS Event" --role node --created-by jan
```

Token deaktivieren/aktivieren/löschen:

```bash
netcore-control-room-operator tokens disable --id tok_...
netcore-control-room-operator tokens enable --id tok_...
netcore-control-room-operator tokens delete --id tok_...
```

## Rollen

```text
node      nur TBS /node WebSocket
viewer    nur lesen
operator  lesen + normale Funkbefehle
admin     alles + Tokenverwaltung + Service Restart/Shutdown
```
