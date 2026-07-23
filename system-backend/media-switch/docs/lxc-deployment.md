# LXC-Deployment

Empfohlen wird ein eigener Debian-LXC mit mindestens zwei vCPU, 512 MiB RAM und einer direkten, latenzarmen Verbindung zum Node Gateway.

Standardports:

- Media Switch WebUI/API: TCP 8130
- Node Gateway Backend-WebSocket: TCP 8080
- Call Control API: TCP 8120

Im Labormodus sind keine Secrets zu verteilen. Die Firewall sollte dennoch nur das interne NetCore-Testnetz zulassen.
