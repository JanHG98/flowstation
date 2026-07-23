# Open-Lab-Modus

Der aktuelle Packet Core hat absichtlich:

- keine Benutzerkonten,
- keine Token,
- keine Session-Authentifizierung,
- kein TLS,
- keine rollenbasierte Freigabe.

Damit kann jeder erreichbare Client Kontexte deaktivieren, Teilnehmer pagen, Zustände modifizieren und Payloads lesen oder einspeisen. Das ist für die offene Testumgebung gewollt, aber für einen produktiven oder fremd erreichbaren Betrieb nicht akzeptabel.

Vor einem produktiven Einsatz müssen mindestens Management-Netztrennung, mTLS oder ein vergleichbarer Dienstidentitätsmechanismus, RBAC und Audit-Trails ergänzt werden.
