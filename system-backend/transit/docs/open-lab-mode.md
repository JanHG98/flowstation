# Open-Lab-Modus

Die aktuelle Phase ist bewusst offen, damit zwei LXC-Regionen ohne Bootstrap-PKI getestet werden können. Management- und Peer-Endpunkte akzeptieren jeden erreichbaren Client.

Daraus folgen harte Betriebsbedingungen:

- eigenes Test-VLAN,
- kein Portforwarding ins Internet,
- Zugriff nur von Laborhosts,
- keine produktiven Schlüssel oder echten Einsatzdaten,
- vor Produktion mTLS, Peer-Zertifikate, RBAC, Audit-Identitäten und signierte Route Advertisements ergänzen.
