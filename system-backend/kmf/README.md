# Key Management Facility

## Zweck

Die KMF verwaltet TETRA-Netz-, Gruppen- und statische Schlüssel.

## Kernaufgaben

- CCK, GCK und SCK verwalten
- Key-Versionen, Crypto Periods und Rotation steuern
- OTAR vorbereiten und ausführen
- Sichere Backups und Schlüssel-Audits führen

## Sicherheitsanforderungen

Strikte Netztrennung, verschlüsselte Datenträger, minimaler Zugriff und spätere HSM-Anbindung.

## WebUI zur Verwaltung

Die KMF erhält eine eigene, ausschließlich im geschützten Managementnetz erreichbare Verwaltungsoberfläche.

### Geplante Ansichten

- Schlüsselmetadaten und Versionen
- CCK-, GCK- und SCK-Zuordnungen ohne Rohschlüsselanzeige
- Crypto Periods und Rotationspläne
- OTAR-Aufträge und Zustellstatus
- Backup-, Restore- und HSM-Zustand
- vollständiges revisionssicheres Audit

### Kritische Aktionen

- Schlüsselgeneration und Rotation
- OTAR-Auftrag freigeben
- Key-Version aktivieren oder zurückziehen
- Backup beziehungsweise Restore durchführen

Rohschlüssel dürfen weder im Browser noch in API-Antworten, Logs oder Exportdateien erscheinen. Kritische Aktionen benötigen Vier-Augen-Freigabe als späteres Ziel.
