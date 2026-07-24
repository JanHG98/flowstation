# Architektur

Der Security Core ist die zentrale Policy- und Authentisierungsinstanz zwischen TBS Edge, Subscriber Core und der späteren KMF.

```text
TBS Edge ── security-edge-v1 ──> Security Core ── Metadaten/Audit ──> WebUI/API
   ▲                                  │
   └──── Challenge/DCK-Aktion ────────┘
                                      │
                                      └── später: KMF Provider
```

## Zuständigkeiten

- Sicherheitsprofile pro ISSI
- Aushandlung der Security Class 1, 2 oder 3
- Challenge/Response-Zustandsmaschine
- kurzlebige DCK-Kontexte und Edge-Installation
- Teilnehmer-/Gerätesperren
- Alarme, Audit und Recovery nach Neustart

## Bewusste Grenze

Der enthaltene `lab_hmac_sha256`-Provider ist ausschließlich ein reproduzierbarer Integrationsprovider. Er ist **kein Ersatz** für die TETRA-Authentisierungsalgorithmen, eine KMF oder zertifizierte Schlüsselhaltung. Die nächste Ausbaustufe stellt Provider-Hooks für K, DCK, CCK, GCK, SCK und OTAR bereit.
