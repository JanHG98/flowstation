# Architektur

## Verantwortungsgrenze

Observability beobachtet alle Core-Dienste, wird aber niemals fachlicher Eigentümer ihrer Teilnehmer-, Gruppen-, Mobility-, Call-, SDS-, Packet- oder Schlüsselzustände. Der Dienst darf bei Ausfall weder Call Control noch Media oder TBS Edge blockieren.

## Zwei Ebenen

1. Der interne Rust-Collector liefert sofort nutzbare Zielzustände, bounded Zeitreihen, JSON-Logs, Trace-Spans, Alarmregeln, Silence/Acknowledge, Audit und Diagnose.
2. Prometheus, Grafana, Loki/Promtail und Alertmanager bilden die langfristige Standardwerkzeugkette. Sie sind getrennte Prozesse und können unabhängig aktualisiert oder ersetzt werden.

## Persistenz

Der Managementzustand wird atomar als JSON geschrieben. Große Langzeitmengen gehören in Prometheus/Loki, nicht in `state.json`. Interne Daten bleiben durch Retention und Mengenlimits begrenzt.
