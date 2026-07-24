# Open-Lab-Modus

Die aktuelle Testphase verwendet absichtlich:

- keine WebUI-Anmeldung,
- keine Management-Tokens,
- kein TLS,
- keine signierten Inbound-Webhooks.

Jeder Client mit Netzweg zum Port 8220 kann Connectoren, Regeln, Vorlagen, Secrets und Dispatches verändern. Der LXC muss deshalb in einem isolierten Management-VLAN betrieben werden.

Connector-Secrets bleiben trotz Open Lab vertrauliche Betriebsdaten. Sie werden separat mit Modus `0600` gespeichert und in Management-Antworten redaktiert.

Vor Produktivbetrieb erforderlich:

- RBAC und Benutzeridentitäten,
- TLS/mTLS,
- signierte beziehungsweise authentisierte Webhooks,
- externes Secret Backend,
- Freigaben für kritische Aussendungen,
- Audit-Export in eine manipulationsgeschützte Ablage.
