# Open-Lab-Modus

Der Media Switch besitzt in diesem Paket absichtlich keine Tokens, Passwörter, Benutzerkonten, Client-Zertifikate oder TLS-Konfiguration.

Jeder Client, der Port 8130 oder den Backend-WebSocket des Node Gateways erreicht, kann Medienströme beobachten und über die Management-API stummschalten, puffern oder Testframes einspeisen. Der Dienst darf daher nur in einem isolierten Testnetz betrieben werden.

`security.mode` akzeptiert ausschließlich `open_lab`. Eine andere Angabe stoppt den Start, statt Scheinsicherheit vorzutäuschen.
