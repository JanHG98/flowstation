# Vault, Backup und HSM

## Lab-File-Vault

Der aktuelle Provider trennt Metadaten und Secret-Blobs und schützt die Blobs mit einem lokalen Master-Key. Er ist für Integrationstests und Datenflussvalidierung gedacht.

Er ist ausdrücklich kein Ersatz für:

- HSM,
- PKCS#11,
- FIPS-/BSI-zertifizierte Kryptografie,
- Hardware-Wurzel des Vertrauens,
- gesicherte Schlüsselzeremonien.

## Backups

Ein Backup enthält:

```text
state.json
vault.json
manifest.json
```

Das Master-Key-File wird nicht kopiert. Ohne getrennt gesicherten Master-Key ist der Vault nicht wiederherstellbar. Das ist Absicht: Metadatenbackup und Schlüsselwurzel sollen nicht im selben Paket liegen.

Das Manifest enthält SHA-256-Prüfsummen und den aktuellen Audit-Head-Hash. Restore ist offline vorgesehen, damit keine laufende Instanz überraschend überschrieben wird.

## HSM-Schnittstelle

Die Konfiguration reserviert bereits:

```toml
[vault]
hsm_library = "/path/to/pkcs11.so"
hsm_slot = 0
```

Im aktuellen Paket bleibt `hsm_connected=false`. Ein späterer Provider muss dieselben Operationen implementieren:

- generate,
- seal/open oder wrap/unwrap,
- destroy,
- fingerprint/reference,
- backup/restore policy,
- health and attestation.
