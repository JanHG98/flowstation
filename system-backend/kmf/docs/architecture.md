# KMF-Architektur

## Zuständigkeit

Die KMF besitzt die autoritative Metadaten- und Lifecycle-Sicht für CCK, GCK und SCK. Der Security Core bleibt zuständig für Security-Class-Policy, Authentisierung, Disable/Enable und kurzlebige DCK-Kontexte.

```text
Security Core                KMF
-------------                ---
Authentication              CCK/GCK/SCK
Security Class              Key Versions
DCK session context         Crypto Periods
Disable/Enable              Rotation
Security alarms             OTAR orchestration
                             encrypted vault/backups
```

## Persistenz

Die Daten werden bewusst getrennt gespeichert:

```text
state.json   Metadaten, Zustände, Audit, Fingerprints
vault.json   nur versiegelte Secret-Blobs
master.key   lokaler Vault-Master-Key, Modus 0600
```

`state.json` kann ohne `master.key` keine Schlüssel offenlegen. `vault.json` enthält keine Klartextschlüssel.

## Secret-Fluss

```text
/dev/urandom
    ↓
CCK/GCK/SCK Klartext nur im Prozessspeicher
    ↓ seal(master.key, context)
vault.json
    ↓ open nur für OTAR-Claim
seal(node_transport_key, action_context)
    ↓
TBS Edge erhält versiegelten Envelope
```

Der Klartext wird weder geloggt noch serialisiert noch in die WebUI gegeben.

## Node-Bootstrap

Für jedes Edge-Node wird ein eigenes Transportprofil erzeugt. Die Bootstrap-Datei enthält das Node-Geheimnis und wird ausschließlich serverseitig mit Modus `0600` geschrieben. Die API liefert lediglich:

- Dateipfad,
- Fingerprint,
- Node-ID,
- Status.

Der Transport-Key wird deterministisch aus Node-Geheimnis und Node-ID abgeleitet. Der KMF-Master-Key schützt ausschließlich die gespeicherte Kopie des Node-Geheimnisses und wird niemals an die Edge verteilt. Damit ist ein OTAR-Envelope an genau dieses Node gebunden.

## Management und Edge-API

Die normale Management-API ist redacted. Der getrennte Edge-Endpunkt liefert keine Rohschlüssel, sondern nur:

- Key-ID und Version,
- Fingerprint,
- Crypto Period,
- Ziele,
- versiegelten Envelope,
- eindeutigen Context-String.

Die Edge muss den Context exakt prüfen und den Envelope mit ihrem Bootstrap-Geheimnis öffnen.
