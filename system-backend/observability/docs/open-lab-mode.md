# Open-Lab-Modus

Der aktuelle Dienst akzeptiert Management- und Ingest-Zugriffe ohne Anmeldung, Token oder TLS. Das ist eine absichtliche Testbedingung und keine produktive Sicherheitsfreigabe.

Pflichtbedingungen:

- isoliertes Management-VLAN,
- kein Portforwarding aus öffentlichen Netzen,
- keine produktiven Geheimnisse in Labels, Logs oder Trace-Attributen,
- Firewallzugriff nur aus Labor- und Adminnetzen,
- vor Produktivbetrieb: TLS/mTLS, zentrale Anmeldung, RBAC und signierte Agentenidentität.
