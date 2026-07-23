# Open-Lab-Modus

Call Control läuft in diesem Paket ausschließlich mit `security.mode = "open_lab"`.

Es existieren keine Tokens, Passwörter, Benutzerkonten, Client-Zertifikate oder TLS-Endpunkte. Jeder Client mit Netzwerkzugriff auf Port 8120 kann Rufe starten und beenden, Floor-Zustände ändern und Restore Context koordinieren.

Andere Security-Modi werden beim Start abgewiesen. Dadurch entsteht keine Scheinsicherheit durch unvollständige Tokenfelder.

Der LXC gehört bis zur späteren Integration von Security Core und RBAC in ein isoliertes Managementnetz beziehungsweise eine Test-VLAN.
