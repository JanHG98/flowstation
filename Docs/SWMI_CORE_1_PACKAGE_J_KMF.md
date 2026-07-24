# SWMI Core 1 – Package J: KMF

## Ziel

Dieses Paket ergänzt nach dem Security Core die zentrale Key Management Facility für:

- CCK,
- GCK,
- SCK,
- Key-Versionen,
- Crypto Periods,
- Rotation,
- OTAR-Orchestrierung,
- sichere Backups,
- spätere HSM-Anbindung.

## Implementierte Runtime

```text
system-backend/kmf/
```

Der Dienst läuft standardmäßig auf Port `8190` und liefert WebUI, REST-API, Liveness, Readiness, Metrics, OpenAPI, systemd-Unit und Installationsskripte.

## Secret-Grenze

Die Management-Ebene enthält niemals Rohschlüssel. Secret-Material liegt getrennt in einem versiegelten Vault. OTAR-Claims liefern ausschließlich einen an das Ziel-Node gebundenen Envelope. Das Bootstrap-Geheimnis wird als lokale Datei mit Modus `0600` geschrieben und nicht per API ausgegeben.

## Lifecycle

```text
Draft → Staged → Active → Retiring → Retired
                     ↘ Revoked
                    → Destroyed
```

Rotation erzeugt einen versionierten Nachfolger mit eigenem Material und verknüpft Vorgänger/Nachfolger. Crypto Periods werden validiert und durch den Wartungslauf ausgewertet.

## OTAR

OTAR-Jobs besitzen:

- Key-Referenz und Fingerprint,
- Ziel-Nodes,
- ISSI-/GSSI-Ziele,
- Not-Before und Ablauf,
- zwei unterschiedliche Freigabe-Actor-Namen,
- Delivery-State pro Node,
- Retry, Timeout und ACK,
- Shadow-/Authoritative-Betrieb.

Die eigentliche D-OTAR-Air-Interface-Codierung bleibt bewusst ein späterer Funkstack-Baustein.

## Vault und Backup

`lab_file_vault` trennt Metadaten, verschlüsselte Secret-Blobs und Master-Key. Backups enthalten `state.json`, `vault.json` und ein Manifest mit SHA-256-Prüfsummen, jedoch nicht den Master-Key.

## Nicht als Produktion ausgeben

- keine Benutzeranmeldung, Tokens oder TLS,
- keine echte Vier-Augen-Identitätsprüfung,
- kein HSM/PKCS#11,
- keine zertifizierte Kryptografie,
- keine TETRA-TA-Algorithmen,
- keine D-OTAR-PDUs.

Diese Grenzen sind in WebUI, Logs, README und Konfiguration sichtbar markiert.
