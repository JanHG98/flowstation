# NetCore Control Room Native UI v4.7 – Directory-first Cleanup

Dieser Stand macht das Directory zur Stammdatenquelle:

- LXC-Core liefert `GET /api/directory` aus `[directory]` in `/etc/netcore-control-room/control-room.toml`.
- Windows-UI zieht dieses Directory automatisch und merged lokale `operator.toml`-Overrides darüber.
- Teilnehmer-Tab zeigt Live-Geräte plus bekannte Directory-Geräte, aber keine Infrastruktur/Gateways/Basisstationen.
- Gruppen-Tab zeigt Live-Gruppen plus Directory-Gruppen.
- Karte/Standorte/Markerdetails nutzen Directory-Namen, Typen, Statusgruppen und Gruppen auch dann, wenn kein Live-Teilnehmerobjekt vorhanden ist.
- Unbekannte Statusnummern werden nicht mehr roh angezeigt.

Keine Patch-Dateien, kompletter Dateistand.
