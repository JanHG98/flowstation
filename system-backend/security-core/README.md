# Security Core

## Zweck

Der Security Core verwaltet Authentisierung, Sicherheitsrichtlinien und Sicherheitszustände.

## Kernaufgaben

- Teilnehmer- und Infrastrukturauthentisierung
- Security-Class-Verhandlung und Sicherheitsprofile
- Geräte- und Teilnehmersperren
- Kryptografische Kontexte, Alarme und Audit

## Abgrenzung

Langfristige Netz- und Gruppenschlüssel liegen in der KMF.

## WebUI zur Verwaltung

Der Security Core erhält eine besonders restriktiv geschützte Verwaltungsoberfläche.

### Geplante Ansichten

- Authentisierungsereignisse und Fehler
- Security Classes und Teilnehmerprofile
- gesperrte beziehungsweise deaktivierte Geräte
- Richtlinien, Crypto-Context-Metadaten und Audit
- Abhängigkeit zur KMF

### Kritische Aktionen

- Gerät oder Teilnehmer sperren und freigeben
- Security Policy ändern
- Authentisierungskontext widerrufen
- Sicherheitsalarm quittieren

Die WebUI zeigt niemals Rohschlüssel oder geheime Challenge-Materialien an. Erhöhte Rechte, erneute Anmeldung und optional MFA sind für schreibende Aktionen vorgesehen.
