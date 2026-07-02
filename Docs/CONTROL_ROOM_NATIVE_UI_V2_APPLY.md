# NetCore Control Room Native UI v2 – Multi-Window + Karte

Dieser ZIP-Stand enthält vollständige Dateien, keine Patch-Dateien.

## Ziel

- Native Windows-/Desktop-UI bleibt der einzige grafische Client.
- Basisstation behält ihren Node-Token in der `config.toml`.
- LXC bleibt headless: Core-Service + CLI/Operator-Tool, kein UI.
- Windows-Operator-PC bekommt die native Leitstellenoberfläche.
- Jedes Modul kann zusätzlich als eigenes verschiebbares/resizables Modulfenster geöffnet werden.
- Standortdaten aus `/api/locations` werden in einer Offline-Karte angezeigt.

## Neue UI-Funktionen

- Neuer Tab `Karte`.
- `Standorte` zeigt jetzt Karte + Tabelle.
- Linke Modulleiste hat pro Modul einen `↗` Button zum Öffnen als eigenes Fenster.
- `Fenster-Modus` kann mehrere Module parallel anzeigen.
- `Alle Module öffnen` öffnet alle wichtigen Module als einzelne Modulfenster.
- `Alle Modulfenster schließen` räumt die Oberfläche wieder auf.
- Karte ist offline/schematisch, nutzt keine externen Tile-Server.

## Wichtig zur Architektur

- Auf dem LXC wird kein grafisches UI installiert oder gestartet.
- Der LXC läuft weiter nur mit `netcore-control-room` und optional CLI.
- Die TBS braucht weiterhin ihren Node-Token in der `config.toml`.
- Windows nutzt einen Admin-/Operator-Token in der lokalen UI-Config oder Token-Datei.

## Windows-Installation / Update

Voraussetzung: Repo ist bereits geklont und CMD steht im Repo-Ordner.

### 1. ZIP entpacken

Wenn die ZIP im Download-Ordner liegt:

```cmd
powershell -NoProfile -Command "Expand-Archive -Force '%USERPROFILE%\Downloads\netcore-control-room-native-ui-v2-multiwindow-map-files.zip' -DestinationPath '%CD%'"
```

Oder Pfad entsprechend anpassen.

### 2. UI neu bauen

```cmd
cargo build --release --manifest-path system-backend\control-room\ui\Cargo.toml
```

Optional Operator-CLI mitbauen:

```cmd
cargo build --release -p netcore-control-room-operator
```

### 3. UI starten

```cmd
system-backend\control-room\ui\target\release\netcore-control-room-ui.exe --config "%APPDATA%\netcore\control-room\operator.toml" --profile default
```

Oder per vorhandener `start-ui.cmd`.

## Bedienung

- In der linken Modulleiste kannst du weiter normal Module auswählen.
- Mit `↗` öffnest du ein Modul zusätzlich als Fenster.
- Mit `▣` erkennst du, dass das Modulfenster offen ist.
- `Alle Module öffnen` öffnet Übersicht, Teilnehmer, Gruppen, Rufe, SDS, Standorte, Karte, Commands und Admin/Tokens parallel.
- `Raw JSON` wird nicht automatisch mit geöffnet, kann aber manuell geöffnet werden.

## Karte

Die Karte ist absichtlich offline und unabhängig von Internetdiensten.

Sie zeigt:

- alle bekannten Standortpunkte aus `/api/locations`
- ISSI als Label am Punkt
- automatisch berechneten Ausschnitt über alle Punkte
- Tabelle der Standortmeldungen darunter

Wenn keine LIP-/Standortdaten vorliegen, zeigt sie eine leere Kartenfläche mit Hinweis.

## Test

1. UI starten.
2. Übersicht muss die TBS `SRV-M_TBS-01` zeigen.
3. `Karte` öffnen.
4. Falls noch keine Standortmeldungen da sind: leere Karte ist korrekt.
5. Ein Funkgerät mit LIP/SDS-Standortmeldung senden lassen.
6. Nach Refresh sollte ein Punkt in der Karte erscheinen.
7. `↗` bei Karte drücken, Karte als Modulfenster offen lassen.
8. Parallel z. B. Übersicht und Commands als eigene Fenster öffnen.

## Build-Hinweis

Dieser Stand wurde als vollständiger Dateistand erzeugt. In der ChatGPT-Sandbox war kein Rust/Cargo installiert, daher wurde der Build hier nicht lokal ausgeführt.
Wenn der Build fehlschlägt, den kompletten Fehler schicken. Dann gibt es wieder einen neuen vollständigen ZIP-Stand, keine Einzel-Fixes.
