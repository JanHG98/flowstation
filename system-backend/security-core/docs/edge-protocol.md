# Security Edge Protocol v1

Der normale Management-Endpunkt zeigt ausschließlich Metadaten. Rohmaterial wird nur über den getrennten Edge-Pfad ausgegeben.

## Ablauf

1. `POST /api/v1/auth/start`
2. Edge holt Aktionen mit `POST /api/v1/edge/actions/claim`
3. Edge setzt Challenge beziehungsweise DCK lokal um
4. Edge bestätigt mit `POST /api/v1/edge/actions/{id}/ack`
5. Authentisierungsantwort an `POST /api/v1/auth/{context_id}/response`

## Regeln

- Claim ist an `node_id` gebunden.
- Eine Aktion besitzt Sequenz, TTL und Zustandswechsel `pending → in_flight → applied|failed`.
- Challenge- und DCK-Payloads werden nicht persistiert.
- Ein erfolgreicher ACK entfernt das Payload aus dem Arbeitsspeicher.
- Der Pfad ist im Open Lab nicht authentisiert und darf daher nur in einem isolierten Managementnetz erreichbar sein.
