# Basisstationen

Der Directory-Bereich „Basisstationen“ hält Metadaten zu einzelnen RF-Standorten oder NetCore-Nodes.

## Felder

| Feld | Bedeutung |
|---|---|
| `issi` | System-/Basisstations-ISSI |
| `name` | vollständiger Standortname |
| `short` | Kurzbezeichnung |
| `location` | Standortbeschreibung |
| `mcc` / `mnc` | Netzkennung |
| `color` | Darstellungsfarbe |
| `visible` | Sichtbarkeit |
| `notes` | interne Hinweise |

## Abgrenzung

Der Directory-Eintrag konfiguriert nicht automatisch RF-Parameter. Carrier, Duplex, Colour Code und Location Area stehen weiterhin in der lokalen `config.toml` der jeweiligen Basisstation.

## Benennung

Eine praktikable Struktur ist:

```text
Name:      Hannover – Rack 01
Kurzname:  H-R01
Standort:  Technikraum Nord
```

Hostnamen können zusätzlich in `node_id` oder `station_name` der Control-Room-Konfiguration geführt werden. Directory-Name und Node-ID sollten stabil und eindeutig sein.
