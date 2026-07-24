# Open-Lab-Modus

Die Management-Schnittstelle ist absichtlich offen. Das ist eine Projektbedingung der aktuellen Testumgebung und keine Produktionsfreigabe.

Besonders kritisch sind:

- URL-Import, weil der Dienst interne HTTP-Ziele erreichen kann,
- Freigabe und Aussendung,
- Löschen lokaler Assets,
- konfigurierbare Codec-Helfer,
- NFS-Archivzugriff.

Die Codec-Befehle sind nur in der rootgeschützten TOML konfigurierbar und nicht per API änderbar. URL-Credentials werden abgelehnt. Für Produktion sind Authentisierung, RBAC, TLS/mTLS, erlaubte Import-Hosts und getrennte Freigaberollen erforderlich.
