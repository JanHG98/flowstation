# Zwei-Regionen-Test

1. Region A auf Port 8200 mit `region_id = "region-a"` starten.
2. Region B auf Port 8210 mit `region_id = "region-b"` starten und den advertised endpoint anpassen.
3. Gegenseitig Peers anlegen.
4. Pro Region eine Default-Route über den jeweiligen Peer anlegen.
5. Auf beiden Seiten zunächst `shadow` verwenden und Route Resolve testen.
6. Danach `authoritative` aktivieren.
7. Eine ISSI auf Region B registrieren.
8. In Region A einen SDS-Transitauftrag an diese ISSI senden.
9. Region B muss eine Local Delivery erzeugen.
10. Delivery quittieren und Session/Events prüfen.
11. Zweiten Peerpfad ergänzen, primären Peer sperren und kontrollierten sowie automatischen Failover prüfen.
12. Einen Envelope mit Region A bereits im Trace einspeisen; er muss als Loop verworfen werden.
