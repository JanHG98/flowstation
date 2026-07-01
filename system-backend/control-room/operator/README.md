# NetCore Control Room Operator

Native Operator-Konsole für den NetCore Control Room. Keine Web-App.

## Grundidee

Der Operator kann weiter klassisch mit `--api` und `--token` genutzt werden. Für den Alltag ist jetzt aber ein lokales Profil besser:

- keine Tokens mehr dauernd in der Shell-History
- Standard-API pro Gerät
- Standard-Node pro Standort
- Standard-Operator-ID für Audit-Logs
- mehrere Profile, z. B. `default`, `event`, `test`

## Profil anlegen

Systemweites Profil, passend für Control-Room-LXC oder festes Leitstellen-Terminal:

```bash
netcore-control-room-operator profiles init \
  --system \
  --profile default \
  --api http://10.0.1.25:9010 \
  --default-node SRV-M_TBS-01 \
  --operator-id jan
```

Dann Token eintragen:

```bash
nano /etc/netcore-control-room/operator.toml
chmod 600 /etc/netcore-control-room/operator.toml
```

Oder Token lieber in eigener Datei halten:

```bash
install -m 600 -o root -g root /dev/null /etc/netcore-control-room/operator.token
nano /etc/netcore-control-room/operator.token

netcore-control-room-operator profiles init \
  --system \
  --force \
  --profile default \
  --api http://10.0.1.25:9010 \
  --token-file /etc/netcore-control-room/operator.token \
  --default-node SRV-M_TBS-01 \
  --operator-id jan
```

## Profil prüfen

```bash
netcore-control-room-operator profiles show
```

Der Token wird dabei nie angezeigt, nur ob einer gefunden wurde und woher.

## Start

Mit Profil reicht:

```bash
netcore-control-room-operator dashboard
```

Oder direkt:

```bash
netcore-control-room-operator overview
netcore-control-room-operator subscribers --online
netcore-control-room-operator groups
netcore-control-room-operator calls
netcore-control-room-operator locations
netcore-control-room-operator sds --limit 20
netcore-control-room-operator commands --limit 20
```

## Alternative: Env/CLI

```bash
export NETCORE_CONTROL_ROOM_API=http://10.0.1.25:9010
export NETCORE_CONTROL_ROOM_TOKEN=<token>

netcore-control-room-operator dashboard
```

Oder:

```bash
netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --token-file /etc/netcore-control-room/operator.token \
  dashboard
```

## Commands

Wenn `default_node` und `operator_id` im Profil gesetzt sind, reicht:

```bash
netcore-control-room-operator kick --issi 2010002
netcore-control-room-operator dgna --issi 2020004 --gssi 15205
netcore-control-room-operator dgna --issi 2020004 --gssi 15205 --detach
netcore-control-room-operator clear-emergency
```

Manuell überschreiben geht weiter:

```bash
netcore-control-room-operator kick --node SRV-M_TBS-01 --issi 2010002 --operator jan
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
