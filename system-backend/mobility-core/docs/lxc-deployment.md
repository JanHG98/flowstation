# LXC-Betrieb

Empfehlung für das Testsystem:

- Debian 12 LXC,
- 1 bis 2 vCPU,
- 512 MiB bis 1 GiB RAM,
- feste IP im isolierten Backend-VLAN,
- ausgehender Zugriff auf den Node Gateway,
- TCP 8090 aus dem Managementnetz.

Der Mobility Core kann in einem separaten LXC vom Node Gateway laufen. In `/etc/netcore/mobility-core.toml` wird dessen WebSocket-Adresse eingetragen.
