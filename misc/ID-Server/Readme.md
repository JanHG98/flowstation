NetCore Directory Server 0.1.0
================================

Lokaler RadioID-ähnlicher Server für NetCore-Tetra / FlowStation.

Start lokal:
  python3 netcore_directory_server.py --host 0.0.0.0 --port 8095 --db ./netcore_directory.db --seed seed.json

Web UI:
  http://<basisstation>:8095/

RadioID-kompatible Tests:
  curl -s 'http://127.0.0.1:8095/api/dmr/user/?id=2020001' | jq .
  curl -s 'http://127.0.0.1:8095/api/dmr/repeater/?id=4010001' | jq .

Native APIs:
  GET    /api/devices
  POST   /api/devices
  GET    /api/devices/<issi>
  PUT    /api/devices/<issi>
  DELETE /api/devices/<issi>

  GET/POST/PUT/DELETE entsprechend:
  /api/basestations
  /api/groups
  /api/status

Systemd:
  sudo mkdir -p /opt/netcore-directory
  sudo cp netcore_directory_server.py /opt/netcore-directory/
  sudo cp seed.json /opt/netcore-directory/
  cd /opt/netcore-directory
  sudo python3 netcore_directory_server.py --db /opt/netcore-directory/netcore_directory.db --seed seed.json
  # mit Strg+C stoppen, danach:
  sudo cp netcore-directory.service /etc/systemd/system/
  sudo systemctl daemon-reload
  sudo systemctl enable --now netcore-directory
