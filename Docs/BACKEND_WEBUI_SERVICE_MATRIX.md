# Backend-WebUI-Service-Matrix

| Dienst | Schwerpunkt der eigenen WebUI | Besonders geschützte Aktionen |
| --- | --- | --- |
| Node Gateway | TBS-Sessions, Heartbeats, Protokollversionen, Backend-Transport | Node trennen und Kommandos senden; im ersten Testpaket bewusst offen ohne Tokens |
| Subscriber Core | Teilnehmer, Geräte, Profile, Berechtigungen | Sperren, Import, Gerätezuordnung |
| Group Core | GSSI, Mitglieder, Affiliationen, DGNA | DGNA, Gruppenrechte, Löschung |
| Mobility Core | Registrierungen, Zellen, Migration, Recovery | Kontextfreigabe, Handover-Abbruch |
| Call Control | Calls, Legs, Floor, Priorität, Restore | Call beenden, Floor entziehen, Pre-emption |
| Media Switch | Streams, Jitter, Routing, Gateways | Stream trennen, Route ändern |
| SDS Router | Nachrichten, Queues, Zustelltrace | Nachricht senden, Retry, Queue löschen |
| Packet Core | PDP Contexts, NSAPI, Durchsatz | Context trennen, Zugriff sperren |
| IP Gateway | Routing, NAT, Firewall, Capture | Firewall ändern, Capture starten |
| Security Core | Authentisierung, Policies, Sperren | Security Policy, Kontextwiderruf |
| KMF | Key-Metadaten, OTAR, Crypto Periods | Rotation, OTAR, Restore; keine Rohschlüsselanzeige |
| Transit | Regionen, Peers, Routen, Failover | Peer sperren, Failover, Route ändern |
| Control Room | Operatoren, Arbeitsplätze, Backend-Verknüpfung | Rollen, Tokens, Leitstellenkonfiguration |
| Application Gateway | Connectoren, Queues, Webhooks | Connector aktivieren, Secrets ersetzen |
| Media Library | Audio, TTS, Vorlagen, Storage | Upload, Löschen, Freigabe |
| Recorder | Aufnahmen, Suche, Retention, Integrität | Export, Retention, Löschung |
| Observability | Metriken, Logs, Traces, Alarme | Alarmregeln, Retention, Stummschaltung |
| Shared | kein Container; gemeinsames UI-Kit | nicht zutreffend |

## Gemeinsame Seiten

Unabhängig vom fachlichen Schwerpunkt besitzt jeder deploybare Dienst die Seiten:

```text
Übersicht
Fachverwaltung
Zustand & Abhängigkeiten
Ereignisse & Audit
Konfiguration
Wartung
API
Über
```
