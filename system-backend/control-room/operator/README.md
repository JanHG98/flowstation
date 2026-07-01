# NetCore Control Room Operator CLI

Ab v5.0 nutzt die CLI klassischen User/Passwort-Login per HTTP Basic Auth.

Beispiel:

```bash
./target/release/netcore-control-room-operator \
  --api http://10.0.1.25:9010 \
  --username jan \
  --password '<passwort>' \
  overview
```

Passwort besser per Datei/Env:

```bash
export NETCORE_CONTROL_ROOM_USER=jan
export NETCORE_CONTROL_ROOM_PASSWORD='<passwort>'
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 overview
```

Benutzerverwaltung:

```bash
./target/release/netcore-control-room-operator users list
./target/release/netcore-control-room-operator users create --username operator1 --password '<pw>' --role operator --display-name 'Operator 1'
./target/release/netcore-control-room-operator users disable --username operator1
./target/release/netcore-control-room-operator users enable --username operator1
./target/release/netcore-control-room-operator users password --username operator1 --password '<neu>'
./target/release/netcore-control-room-operator users delete --username operator1
```
