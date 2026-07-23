# Open-Lab-Modus

Diese Recorder-Ausbaustufe akzeptiert ausschließlich `security.mode = "open_lab"`. Ein anderer Wert beendet den Start mit einem Konfigurationsfehler.

Es gibt bewusst keine Authentifizierung, Tokenprüfung, Benutzerverwaltung, Zertifikate oder TLS-Terminierung. Die WebUI zeigt dauerhaft einen roten Warnbalken. Auch HTTP-Antworten tragen `X-NetCore-Security-Mode: open_lab`.

## Konsequenz

Jeder erreichbare Client kann Aufnahmen lesen und exportieren sowie – abhängig von `allow_remote_management` und `allow_delete` – Retention, Legal Hold, Finalisierung und Löschung steuern.

## Mindestschutz im Testnetz

- eigener LXC
- eigenes isoliertes Backend-/Management-VLAN
- kein Port-Forwarding aus dem Internet
- Firewall nur für Administratoren und die Leitstelle
- Storage nicht gleichzeitig als ungeschütztes SMB/NFS veröffentlichen

Die spätere Security-Phase muss Authentifizierung, Autorisierung, Audit-Identitäten und Transportverschlüsselung ergänzen. Sie ist absichtlich nicht vorgezogen worden.
