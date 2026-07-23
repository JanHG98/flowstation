# SWMI Core 1 – Paket A: Subscriber Core

Dieses Paket ergänzt den dritten LXC-Dienst `system-backend/subscriber-core`.

## Enthalten

- persistente, atomar gespeicherte Teilnehmerdatenbank
- Teilnehmer-CRUD, JSON-Import/Export und CSV-Export
- offene WebUI auf Port 8100
- Node-Gateway-Backend-Anbindung
- versionierte TBS-Zugangsrichtlinie
- explizite Unterscheidung zwischen Open Network und leerer Allow-List/Deny-All
- TBS-Bestätigung, Timeout und Synchronisationszustände
- Live-Registrierungsansicht aus TBS-Telemetrie

Die Teststufe enthält bewusst keine Tokens, Benutzerkonten oder TLS.
## Migrierte Teilnehmer und VASSI

Die zentrale Allow-List enthält Home-ISSIs. Eine TBS kann einen migrierten Teilnehmer lokal jedoch unter einer VASSI führen. Paket A hält deshalb eine langlebige Zuordnung `lokale VASSI -> Home-ISSI`, die bewusst länger lebt als die begrenzte MM-Transaktionshistorie. Registrierung, Gruppenaffiliation und das Trennen nicht mehr berechtigter Teilnehmer werden dadurch gegen die Home-Identität geprüft.

Die reine Live-Telemetrie zeigt bis zur späteren engeren Subscriber-/Mobility-Core-Kopplung weiterhin die lokal auf der Luftschnittstelle verwendete SSI an.

