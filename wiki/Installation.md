# Installation

Die folgenden Schritte beschreiben eine quellbasierte Installation auf Debian oder Raspberry Pi OS. Pfade und Benutzer sind Beispiele und müssen zum Zielsystem passen.

## Voraussetzungen

- 64-Bit Linux auf Raspberry Pi 4/5 oder vergleichbarer Hardware
- Rust-Toolchain mit Cargo
- C/C++-Buildwerkzeuge und `pkg-config`
- SoapySDR und der passende SDR-Gerätetreiber
- Git
- `ffmpeg`, wenn Audio-Zentrale, MP3/WAV-Wiedergabe oder TTS genutzt werden
- nativer TETRA-Sprachcodec, wenn Asterisk oder Audiofunktionen gebaut werden

```bash
sudo apt update
sudo apt install -y \
  git curl build-essential pkg-config cmake clang \
  libsoapysdr-dev soapysdr-tools ffmpeg
```

Rust installieren:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup update stable
```

## Repository bereitstellen

```bash
cd ~
git clone <REPOSITORY-URL> netcore
cd ~/netcore
```

Bei einem bereits vorhandenen Arbeitsverzeichnis zuerst prüfen:

```bash
git status
git branch --show-current
git remote -v
```

Lokale Änderungen niemals blind mit `reset --hard` verwerfen. Zuerst sichern oder committen.

## SDR prüfen

```bash
SoapySDRUtil --info
SoapySDRUtil --find
SoapySDRUtil --probe="driver=<TREIBER>"
```

Die Basisstation sollte erst gestartet werden, wenn das SDR zuverlässig erkannt wird und die konfigurierte Sample-Rate unterstützt.

## Konfiguration anlegen

Die Basisstation erwartet beim Start einen Pfad zur Konfiguration:

```bash
cp config.toml config.local.toml
chmod 600 config.local.toml
```

Zugangsdaten, Tokens und standortspezifische Frequenzen nur in der lokalen Datei pflegen. Hinweise zu den Sektionen stehen unter [[Configuration]].

## Build

Der Standard-Build enthält Asterisk, Recording und Audio-Player:

```bash
cargo clean
rm -rf target
cargo build --release -p bluestation-bs
```

Die Binärdatei liegt anschließend unter:

```text
target/release/bluestation-bs
```

Minimaler Build ohne Standard-Medienfunktionen:

```bash
cargo clean
rm -rf target
cargo build --release -p bluestation-bs --no-default-features
```

## Erster manueller Start

```bash
RUST_LOG=info ./target/release/bluestation-bs ./config.local.toml
```

Prüfen:

- Konfiguration wird ohne Fallback geladen.
- SDR und Center-Frequenzen sind korrekt.
- Downlink startet stabil.
- Dashboard bindet auf der erwarteten Adresse und dem erwarteten Port.
- Keine dauerhaften Buffer-, Timing- oder Passband-Fehler erscheinen.

Mit `Ctrl+C` sauber beenden und erst danach den Systemd-Dienst einrichten.

## Zusätzliche Dienste

- [[NetCore-Directory]] für Namen, Status und Statusgruppen
- [[Audio-Zentrale]] und NetCore Piper für TTS
- [[Control-Room]] für den zentralen Leitstellenbetrieb
- NFS-Mount für Aufnahme- und TTS-Archive
