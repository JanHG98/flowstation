# NetCore Control Room Operator

Native operator console for the NetCore Control Room.

This is intentionally **not** a web app. It is a standalone executable that connects to a running `netcore-control-room` core service by HTTP API.

## Build

From the repository root:

```bash
cargo build --release -p netcore-control-room-operator
```

## Run dashboard

```bash
./target/release/netcore-control-room-operator \
  --api http://10.10.40.20:9010 \
  dashboard
```

or with an environment variable:

```bash
export NETCORE_CONTROL_ROOM_API=http://10.10.40.20:9010
./target/release/netcore-control-room-operator dashboard
```

## Useful commands

```bash
netcore-control-room-operator --api http://10.10.40.20:9010 overview
netcore-control-room-operator --api http://10.10.40.20:9010 subscribers --online
netcore-control-room-operator --api http://10.10.40.20:9010 groups
netcore-control-room-operator --api http://10.10.40.20:9010 calls
netcore-control-room-operator --api http://10.10.40.20:9010 locations
netcore-control-room-operator --api http://10.10.40.20:9010 sds --limit 20
```

Send commands:

```bash
netcore-control-room-operator --api http://10.10.40.20:9010 kick --node tbs-04010001 --issi 2010002
netcore-control-room-operator --api http://10.10.40.20:9010 dgna --node tbs-04010001 --issi 2020004 --gssi 15205
netcore-control-room-operator --api http://10.10.40.20:9010 dgna --node tbs-04010001 --issi 2020004 --gssi 15205 --detach
netcore-control-room-operator --api http://10.10.40.20:9010 clear-emergency --node tbs-04010001
```


## Auth

Wenn der Control-Room-Core Auth aktiviert hat, nutze entweder `--token` oder die Umgebungvariable `NETCORE_CONTROL_ROOM_OPERATOR_TOKEN`:

```bash
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 --token <operator-token> dashboard
```

```bash
export NETCORE_CONTROL_ROOM_OPERATOR_TOKEN=<operator-token>
./target/release/netcore-control-room-operator --api http://10.0.1.25:9010 dashboard
```
