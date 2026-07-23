# Open-Lab-Modus

Dieser Ausbaustand enthält absichtlich noch keine Tokens, Benutzerkonten, Rollen oder TLS. Das ist keine Produktivkonfiguration.

Besonders kritisch ist beim SDS Router, dass jeder erreichbare Client:

- SDS- und Statusinhalte lesen,
- neue Nachrichten aussenden,
- Routingregeln ändern,
- Offline- und Dead-Letter-Nachrichten erneut zustellen,
- Anwendungslegs bestätigen kann.

Der Dienst muss daher in einem isolierten Labor-VLAN betrieben werden. Vor einem produktiven Einsatz sind mindestens mTLS oder signierte Dienstidentitäten, RBAC, Audit-Trails, Payload-Masking und verschlüsselte Datenträger vorzusehen.
