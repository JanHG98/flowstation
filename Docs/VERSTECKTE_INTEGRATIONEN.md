# Versteckte Integrationen

Die folgenden Integrationen sind im normalen Dashboard ausgeblendet und nur über einen direkten Aufruf erreichbar.

## Geheimmodus aktivieren

Ersetze `<BASISSTATION-IP>` und `<PORT>` durch die Adresse deiner Basisstation.

### DAPNET

```text
http://<BASISSTATION-IP>:<PORT>/?intern=netcore&modul=dapnet
```

### EchoLink

```text
http://<BASISSTATION-IP>:<PORT>/?intern=netcore&modul=echolink
```

### MeshCom

```text
http://<BASISSTATION-IP>:<PORT>/?intern=netcore&modul=meshcom
```

### GeoAlarm

```text
http://<BASISSTATION-IP>:<PORT>/?intern=netcore&modul=geoalarm
```

Nach dem Aufruf wird der Geheimmodus für den aktuellen Browser-Tab aktiviert. Anschließend sind alle versteckten Integrationen im Dashboard sichtbar.

Der Parameter wird danach automatisch aus der Adresszeile entfernt.

## Geheimmodus deaktivieren

```text
http://<BASISSTATION-IP>:<PORT>/?intern=off
```

Alternativ kann der betreffende Browser-Tab geschlossen werden.

## Sicherheitshinweis

Der Geheimmodus ist lediglich eine versteckte Darstellung innerhalb des Dashboards und keine echte Zugriffskontrolle.

Die zugehörigen Backend-Funktionen und API-Endpunkte bleiben erreichbar, sofern sie nicht zusätzlich durch Firewall, Reverse Proxy oder Authentifizierung geschützt werden.
