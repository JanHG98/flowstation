# Häufige Buildfehler

## `pkg-config` findet SoapySDR nicht

```bash
pkg-config --modversion SoapySDR
sudo apt install libsoapysdr-dev pkg-config
```

Bei selbst installierten Bibliotheken `PKG_CONFIG_PATH` und Architekturpfad prüfen.

## Native Sprachcodec-Bibliothek fehlt

Asterisk, Recorder und Audio-Player können native Codec-Abhängigkeiten benötigen. Prüfen:

```bash
pkg-config --list-all | grep -i tetra
ldconfig -p | grep -i tetra
```

Danach immer Clean-Build.

## Linkerfehler nach Featurewechsel

```bash
cargo clean
rm -rf target
cargo build --release -p bluestation-bs
```

Nicht versuchen, alte Artefakte mit einzelnen Dateilöschungen zu retten.

## Rust-Version zu alt

Das Workspace nutzt Edition 2024.

```bash
rustup update stable
rustc --version
cargo --version
```

## Build des ganzen Workspace zieht unnötige Abhängigkeiten

Auf einem Leitstellenserver gezielt bauen:

```bash
cargo build --release -p netcore-control-room
cargo build --release -p netcore-control-room-operator
```

Auf der Basisstation:

```bash
cargo build --release -p bluestation-bs
```

## Speicherplatz reicht nicht

```bash
df -h
sudo du -sh target ~/.cargo/registry ~/.cargo/git 2>/dev/null
```

`target` kann gefahrlos nach gestopptem Build gelöscht werden. Cargo-Caches nur bewusst bereinigen, da sie später erneut heruntergeladen bzw. gebaut werden.

## Fehler erst zur Laufzeit

Ein erfolgreicher Build beweist nicht, dass der dynamische Loader alle Bibliotheken findet:

```bash
ldd target/release/bluestation-bs | grep 'not found'
```
