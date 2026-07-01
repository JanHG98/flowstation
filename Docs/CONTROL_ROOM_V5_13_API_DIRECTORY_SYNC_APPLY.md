# NetCore Control Room v5.13.0 – API Directory Sync

Dieses Update beendet die lokale TOML-Namensraterei.

## Architektur

- LXC `/api/directory` ist jetzt dynamisch.
- LXC kann Directory-Daten per API importieren:
  - `POST /api/directory/import`
  - Alias: `POST /api/directory/merge`
- LXC liefert eine aufgelöste Sicht:
  - `GET /api/directory/resolved`
- Windows-UI nutzt zuerst `/api/directory/resolved`.
- Status-Tableau und Karte bekommen Namen aus dieser API-Sicht.

## Warum

Dein aktueller LXC lieferte:

```json
{
  "hide_infrastructure": true
}
```

Damit kann die Windows-UI keine Namen anzeigen, auch wenn die Basisstation sie lokal kennt.
v5.13 schafft jetzt den API-Kanal, über den diese Namen in den LXC kommen.

## LXC installieren

```bash
cd /opt/netcore/flowstation

systemctl stop netcore-control-room || true

cargo clean -p netcore-control-room
cargo build --release \
  -p netcore-control-room \
  -p netcore-control-room-operator

systemctl daemon-reload
systemctl start netcore-control-room
journalctl -u netcore-control-room -f
```

## Import testen

Beispiel mit Datei:

```bash
cat >/tmp/directory.json <<'JSON'
{
  "subscribers": {
    "2020001": { "name": "Birke HRT", "device_class": "HRT", "status_group": "funk" },
    "2020002": { "name": "Ader HRT", "device_class": "HRT", "status_group": "funk" }
  },
  "status_groups": {
    "funk": { "name": "Funkgeräte" }
  },
  "statuses": {
    "1": { "label": "Frei" },
    "2": { "label": "Bereit" },
    "3": { "label": "Sprechwunsch" },
    "4": { "label": "Einsatz" },
    "5": { "label": "Am Ziel" },
    "6": { "label": "Nicht bereit" },
    "7": { "label": "Transport" },
    "8": { "label": "Sonderstatus" }
  },
  "hide_infrastructure": true
}
JSON

curl -u admin:DEIN_PASSWORT \
  -H 'Content-Type: application/json' \
  --data-binary @/tmp/directory.json \
  http://127.0.0.1:9010/api/directory/import | jq
```

Dann prüfen:

```bash
curl -u admin:DEIN_PASSWORT http://127.0.0.1:9010/api/directory/resolved | jq
```

## Wichtig

Das ist noch nicht automatisch aus der Basisstation gezogen. Aber es ist jetzt ein echter API-Pfad.
Der nächste Schritt kann sein: Basisstation sendet beim Start ihr Directory automatisch an `POST /api/directory/import`.
