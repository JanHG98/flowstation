# Directory API

Die API wird von Weboberfläche, Basisstation und optionalen Hilfswerkzeugen genutzt. Beispiele enthalten keine Authentifizierungsdaten und verwenden Platzhalter.

## Basis-Endpunkte

| Methode | Endpunkt | Zweck |
|---|---|---|
| `GET` | `/api/health` | Dienstzustand |
| `GET` | `/api/export` | vollständiger Datenexport |
| `POST` | `/api/import` | Datenimport |
| CRUD | `/api/devices` | Geräte |
| CRUD | `/api/basestations` | Basisstationen |
| CRUD | `/api/groups` | Gruppen |
| CRUD | `/api/device-groups` | Gerätegruppen/Statusgruppen |
| CRUD | `/api/status` | Statusmeldungen |
| `GET` | `/api/status-group-members?issi=<ISSI>` | Statusgruppen eines Geräts und Mitglieder |

Je nach Client stehen Alias-Endpunkte für `status-groups` oder `vehicles` zur Verfügung.

## RadioID-kompatible Abfragen

```text
GET /api/dmr/user/?id=<ISSI>
GET /api/dmr/repeater/?id=<ISSI>
```

Diese Routen erleichtern die Wiederverwendung bestehender Lookup-Logik.

## Export

```bash
curl -fsS http://<DIRECTORY-IP>:8095/api/export \
  -o netcore-directory-export.json
```

Exportdateien können personenbezogene oder betriebliche Informationen enthalten und sind entsprechend zu schützen.

## Import

Vor jedem Import Datenbank und bestehenden Export sichern. Beispiel:

```bash
curl -fsS -X POST \
  -H 'Content-Type: application/json' \
  --data-binary @netcore-directory-import.json \
  http://<DIRECTORY-IP>:8095/api/import
```

## Fehlercodes

- `2xx` – erfolgreich
- `400` – ungültige Nutzdaten oder fehlende Pflichtfelder
- `404` – Datensatz oder Route nicht gefunden
- `409` – Konflikt, etwa doppelte eindeutige ID
- `500` – interner Fehler; Serverlog prüfen

## Praxisregel

API-Clients sollten Timeouts setzen und einen Directory-Ausfall nicht mit einem Ausfall der RF-Basisstation gleichsetzen. Namensauflösung darf den kritischen Funkpfad nicht blockieren.
