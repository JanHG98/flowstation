# NetCore-Tetra Backend-WebUI-Standard

## 1. Verbindliche Architekturregel

Jeder eigenständig laufende System-Backend-Dienst erhält eine eigene WebUI zur Verwaltung. Dies gilt unabhängig davon, ob der Dienst später als Proxmox-LXC, VM oder eigenständiger Prozess betrieben wird.

Die WebUI ist Teil des jeweiligen Service-Pakets:

```text
system-backend/<dienst>/
├── src/ beziehungsweise Servercode
├── web-ui/ oder eingebettete UI-Assets
├── config/
├── systemd/
└── README.md
```

Es wird **kein zusätzlicher Frontend-LXC pro Dienst** benötigt.

## 2. Unabhängigkeit

- Die fachliche Runtime muss ohne geöffneten Browser vollständig funktionieren.
- Ein Fehler im UI-Renderer darf Call Control, Mobility, Media, SDS oder Packet Data nicht stoppen.
- Der Control Room darf Service-WebUIs verlinken und zusammenfassen, ist aber keine Voraussetzung für deren Nutzung.
- Jeder Dienst bleibt bei Ausfall des Control Rooms separat administrierbar.

## 3. Einheitlicher Zugriff

Neue LXC-Dienste verwenden langfristig standardmäßig:

```text
https://<LXC-IP>:8443/
```

Da jeder Container eine eigene IP besitzt, kann derselbe Port mehrfach verwendet werden. Bereits vorhandene Dienste dürfen ihren bisherigen Port behalten.

### Vorübergehender Open-Lab-Modus

Während des frühen Testaufbaus darf ein Dienst ausdrücklich als `open_lab` markiert werden. Dann gelten abweichend:

- HTTP statt HTTPS ist zulässig,
- es gibt keine Tokens, Benutzerkonten oder Client-Zertifikate,
- der offene Zustand muss in WebUI, API, Logs und Dokumentation deutlich sichtbar sein,
- der Dienst darf ausschließlich in einem isolierten Test-/Managementnetz erreichbar sein,
- ein nicht implementierter Sicherheitsmodus darf nicht als produktionsfähig ausgegeben werden.

Der erste Node Gateway verwendet daher zunächst `http://<LXC-IP>:8080/`. Diese Ausnahme ist absichtlich und keine Lockerung der späteren Produktivanforderungen.

Empfohlene Endpunkte:

```text
/                  WebUI
/api/v1             versionierte Verwaltungs-API
/openapi.json        API-Beschreibung
/health/live         Prozess lebt
/health/ready        Dienst und Pflichtabhängigkeiten bereit
/metrics             Prometheus-Metriken
```

## 4. Pflichtbereiche jeder WebUI

Jede Oberfläche enthält mindestens:

1. **Übersicht** – Zustand und wichtigste fachliche Kennzahlen.
2. **Fachverwaltung** – die für den Dienst spezifischen Objekte und Aktionen.
3. **Abhängigkeiten** – Erreichbarkeit und Versionen benötigter Dienste.
4. **Ereignisse/Audit** – nachvollziehbare Änderungen und Fehler.
5. **Konfiguration** – validierte, rollenabhängige Einstellungen.
6. **Wartung** – kontrollierter Reload, Drain, Backup oder Diagnose.
7. **API** – dokumentierte Verwaltungsendpunkte.
8. **Über** – Version, Git-Hash, Buildzeit und Protokollversion.

## 5. Authentisierung und Rollen

Geplante gemeinsame Rollen:

- `viewer` – lesen, keine Änderungen
- `operator` – betriebliche Aktionen
- `administrator` – Konfiguration und Benutzerrechte
- `auditor` – Audit und Export, keine Betriebsänderungen

Zusätzliche dienstspezifische Rollen sind zulässig.

Die langfristige Hauptanmeldung soll zentral angebunden werden. Jeder Dienst sieht zusätzlich einen deaktivierbaren lokalen Break-Glass-Zugang für den Notbetrieb vor. Passwörter werden ausschließlich gehasht gespeichert.

## 6. Sicherheitsanforderungen

- HTTPS im Managementnetz
- sichere Session-Cookies
- CSRF-Schutz bei zustandsändernden Browseraktionen
- Content Security Policy
- Eingabevalidierung serverseitig
- Rate Limiting für Login und kritische Aktionen
- keine Zugangsdaten, Tokens oder Schlüssel in HTML, Logs oder Fehlermeldungen
- erneute Bestätigung bei destruktiven Aktionen
- Audit mit Bediener, Zeit, Quelle, Aktion und Ergebnis

Für `security-core` und `kmf` gelten zusätzliche Einschränkungen. Insbesondere werden Rohschlüssel niemals angezeigt oder über die normale Verwaltungs-API ausgegeben.

## 7. Konfigurationsänderungen

Jede Änderung muss klassifiziert werden:

- **live wirksam** – ohne Neustart
- **reload erforderlich** – kontrollierter Konfigurationsreload
- **restart erforderlich** – explizit anzeigen, nie überraschend neu starten
- **nicht online änderbar** – nur über Wartungsworkflow

Vor dem Speichern erfolgt eine Validierung. Fehlgeschlagene Änderungen dürfen die bisherige gültige Konfiguration nicht beschädigen.

## 8. Echtzeitdaten

Live-Ansichten verwenden SSE oder WebSocket. Fachlich kritische Steuerbefehle laufen weiterhin über die versionierte Verwaltungs-API und erhalten eine eindeutige Command-ID sowie ein Ergebnis.

## 9. Gemeinsame UI-Bausteine

Gemeinsamer Code liegt unter:

```text
system-backend/shared/web-ui/
```

Dort entstehen Layout, Authentisierung, Rollen, API-Client, Tabellen, Formulare, Health-Seiten, Audit-Komponenten und Übersetzungen.

## 10. Definition of Done für jeden neuen Dienst

Ein Backend-Dienst gilt erst als vollständig, wenn:

- seine fachliche Runtime funktioniert,
- seine WebUI die Pflichtbereiche bereitstellt,
- Readiness und Liveness vorhanden sind,
- alle Schreibaktionen RBAC und Audit verwenden oder während einer ausdrücklich dokumentierten `open_lab`-Phase eindeutig als ungeschützt gekennzeichnet sind,
- Konfigurationsänderungen validiert werden,
- die UI bei ausgeschaltetem Control Room erreichbar bleibt,
- API- und UI-Tests vorhanden sind,
- Installations- und Updateanleitung die WebUI berücksichtigt.
