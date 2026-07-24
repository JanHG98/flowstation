# Backend-WebUI-Service-Matrix

| Dienst | Schwerpunkt der eigenen WebUI | Besonders geschützte Aktionen |
| --- | --- | --- |
| Node Gateway | TBS-Sessions, Heartbeats, Protokollversionen, Backend-Transport | Node trennen und Kommandos senden; im ersten Testpaket bewusst offen ohne Tokens |
| Subscriber Core | Teilnehmer, Geräte, Profile, Berechtigungen, TBS-Synchronisation | Sperren, Import, Gerätezuordnung; im Testpaket bewusst offen ohne Tokens |
| Group Core | GSSI, Mitglieder, Affiliationen, DGNA | DGNA, Gruppenrechte, Löschung; im Testpaket bewusst offen ohne Tokens |
| Mobility Core | Registrierungen, Zellen, Migration, Recovery | Kontextfreigabe, Handover-Abbruch |
| Call Control | Calls, Legs, Floor, Priorität, Restore | Call beenden, Floor entziehen, Pre-emption; im Testpaket bewusst offen ohne Tokens |
| Media Switch | Streams, Jitter, Routing, TBS-Legs, Recorder-Taps | Stream stummschalten, Puffer leeren, Testframe einspeisen; im Testpaket bewusst offen ohne Tokens |
| SDS Router | Nachrichten, Queues, Zustelltrace | Nachricht senden, Retry, Queue löschen; im Testpaket bewusst offen ohne Tokens |
| Packet Core | PDP Contexts, NSAPI, READY/STANDBY, Bearer, Fragmentierung und Flow Control | Kontext pagen, modifizieren, beenden oder trennen; im Testpaket bewusst offen ohne Tokens |
| IP Gateway | TUN, PDP-IP-Leases, Routing, NAT, Firewall, DNS, WAP/Testdienste, Flows und PCAP | Kernel-Reconcile, Route/NAT/Firewall ändern, Flow blockieren und Capture starten; im Testpaket bewusst offen ohne Tokens |
| Security Core | Authentisierung, Security Classes, DCK-Metadaten, Sperren, Alarm/Audit | Policy, Disable/Enable, Kontext-/DCK-Widerruf; keine Rohschlüsselanzeige |
| KMF | CCK/GCK/SCK, Key-Versionen, Crypto Periods, Rotation, OTAR, Vault und Backup | Rotation, Revoke/Destroy, OTAR-Freigabe und Backup; keine Rohschlüsselanzeige, im Testpaket bewusst offen ohne Tokens |
| Transit | Regionen, Peers, Routen, Failover | Peer sperren, Failover, Route ändern |
| Control Room | Operatoren, Arbeitsplätze, Backend-Verknüpfung | Rollen, Tokens, Leitstellenkonfiguration |
| Application Gateway | Connectoren, Queues, Webhooks | Connector aktivieren, Secrets ersetzen |
| Media Library | Audio, TTS, Vorlagen, Storage | Upload, Löschen, Freigabe |
| Recorder | Aufnahmen, Suche, Retention, Integrität | Export, Retention, Hold und Löschung; im Testpaket bewusst offen ohne Tokens |
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
