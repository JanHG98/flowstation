# Phase 4: TTS nur noch als Vorschau erzeugen und anschließend senden

## Ziel

Der direkte Ablauf **„Erzeugen und senden“** wurde vollständig aus der Dashboard-Oberfläche und der öffentlichen HTTP-API entfernt.

TTS verwendet jetzt ausschließlich diesen zweistufigen Ablauf:

1. **Vorschau erzeugen**
2. Vorschau anhören beziehungsweise prüfen
3. **Diese Vorschau senden**

Damit kann keine noch nicht geprüfte oder noch in der Erzeugung befindliche TTS-Datei direkt in einen Funkruf übergehen.

## Dashboard

Entfernt:

- Schaltfläche `Erzeugen und senden`
- JavaScript-Funktion `dispatchTtsNow()`

Beibehalten:

- Schaltfläche `Vorschau erzeugen`
- Audioplayer für die fertige Vorschau
- Schaltfläche `Diese Vorschau senden`

## HTTP-API

Verfügbar:

- `POST /api/audio/tts/generate`
- `POST /api/audio/tts/send`

Entfernt:

- `POST /api/audio/tts/dispatch`

Ein Aufruf der entfernten Route erhält nun regulär `404 not found`.

## Service-Schnittstelle

Die öffentliche Rust-Methode `generate_and_dispatch()` wurde entfernt. Der Versand kann nur noch über `dispatch_ready()` mit der ID einer vollständig erzeugten Vorschau gestartet werden.

## Unverändert

- Piper-Sprachsynthese
- lokale TTS-Vorlagen
- Vorschau-Audioplayer
- gemeinsamer Recording-/TTS-AudioPlayer
- CMCE-, UMAC- und RF-Logik
- Recordings und Medienbibliothek
