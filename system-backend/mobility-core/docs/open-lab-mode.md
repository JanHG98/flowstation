# Offener Labormodus

Der Mobility Core läuft in dieser Ausbaustufe ausschließlich im Modus `open_lab`.

Es gibt keine Tokens, Benutzerkonten, Passwörter, Client-Zertifikate oder TLS-Verbindung. Jeder Client, der die WebUI oder REST-API erreichen kann, darf Context Transfers auslösen oder abbrechen.

Der Dienst darf deshalb nur in einem isolierten Test- beziehungsweise Managementnetz betrieben werden. Ein anderer Wert als `security.mode = "open_lab"` führt absichtlich zum Startabbruch.
