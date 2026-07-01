# Installation

Diese Seite beschreibt die grundlegende Installation von NetCore-Tetra auf einem Linux-System.

Die Installation besteht aus mehreren Teilen:

1. System vorbereiten
2. Quellcode bereitstellen
3. Abhängigkeiten installieren
4. Basisstation bauen
5. Konfiguration anlegen
6. NetCore Directory einrichten
7. Systemd-Dienste erstellen
8. Start und Funktion prüfen

> **Hinweis:** Die hier beschriebenen Schritte beziehen sich auf eine typische lokale Installation. Pfade, Service-Namen, Benutzer und Hardware können je nach Umgebung abweichen.

---

## Rechtlicher Hinweis

Vor dem Betrieb einer TETRA-Basisstation müssen die rechtlichen und regulatorischen Rahmenbedingungen geprüft werden.

Insbesondere relevant sind:

* zulässige Frequenzbereiche,
* Sendeleistung,
* Antennengewinn,
* Standort,
* Funkdienst,
* Betriebsart,
* Netzkennungen,
* mögliche Störungen anderer Funkdienste.

NetCore-Tetra ist für private, lokale, experimentelle und kontrollierte Umgebungen gedacht. Der Betreiber ist selbst dafür verantwortlich, dass Betrieb und Konfiguration zulässig sind.

---

## Zielsystem

Eine typische Installation besteht aus:

```text id="dh5rhc"
Linux Host
├── NetCore-Tetra Basisstation
├── NetCore Dashboard
├── NetCore Directory
├── config.toml
├── SDR-Hardware
└── systemd Services
```

Empfohlene Umgebung:

```text id="x0mvxm"
Debian / Ubuntu / Raspberry Pi OS
systemd
Rust Toolchain
Python 3
SQLite
SDR mit passender Treiberunterstützung
```

---

## Empfohlene Verzeichnisse

Für eine saubere Installation bietet sich folgende Struktur an:

```text id="saqgxl"
/opt/netcore-tetra/
├── flowstation/              # Quellcode / Build-Verzeichnis
├── config/
│   ├── config.toml
│   └── config.toml.fallback
├── directory/
│   ├── netcore_directory_server.py
│   └── netcore_directory.db
├── logs/
└── backups/
```

Alternativ kann der Quellcode auch direkt im Home-Verzeichnis eines Service-Benutzers liegen.

Beispiel:

```text id="bgen7y"
/home/tetra/flowstation/
/home/tetra/config.toml
/home/tetra/netcore-directory/
```

Wichtig ist, dass die Pfade später in den systemd-Units und in der Konfiguration zusammenpassen.

---

## System vorbereiten

Zuerst das System aktualisieren:

```bash id="k0bpct"
sudo apt update
sudo apt upgrade
```

Benötigte Basispakete installieren:

```bash id="mfibji"
sudo apt install -y \
  git \
  curl \
  build-essential \
  pkg-config \
  cmake \
  clang \
  libssl-dev \
  python3 \
  python3-venv \
  sqlite3 \
  jq
```

Je nach SDR-Hardware werden zusätzliche Pakete benötigt.

Typische SDR-Pakete:

```bash id="pjnavk"
sudo apt install -y \
  soapysdr-tools \
  libsoapysdr-dev
```

Nach der Installation kann die SDR-Erkennung geprüft werden:

```bash id="llz5ho"
SoapySDRUtil --find
SoapySDRUtil --probe
```

---

## Rust installieren

NetCore-Tetra wird aus dem Quellcode gebaut. Dafür wird Rust benötigt.

Installation:

```bash id="p9nd9w"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Danach die Umgebung laden:

```bash id="yc7tq9"
source "$HOME/.cargo/env"
```

Prüfen:

```bash id="zmdg0v"
rustc --version
cargo --version
```

---

## Benutzer anlegen

Es ist sinnvoll, NetCore-Tetra nicht dauerhaft als root zu betreiben.

Beispiel:

```bash id="qcxv1w"
sudo useradd -r -m -d /opt/netcore-tetra -s /bin/bash netcore
```

Verzeichnisrechte setzen:

```bash id="t3t836"
sudo mkdir -p /opt/netcore-tetra
sudo chown -R netcore:netcore /opt/netcore-tetra
```

Für SDR-Zugriff muss der Benutzer je nach System passenden Gruppen angehören.

Beispiele:

```bash id="ygj4bt"
sudo usermod -aG plugdev netcore
sudo usermod -aG dialout netcore
```

Danach einmal abmelden oder System neu starten, damit Gruppenrechte greifen.

---

## Quellcode bereitstellen

In das Installationsverzeichnis wechseln:

```bash id="hvozcq"
sudo -iu netcore
cd /opt/netcore-tetra
```

Repository klonen:

```bash id="yqh4rp"
git clone <REPOSITORY-URL> flowstation
cd flowstation
```

> `<REPOSITORY-URL>` durch die tatsächliche Repository-Adresse ersetzen.

---

## Build der Basisstation

Im Quellcode-Verzeichnis:

```bash id="ifsqkc"
cargo build --release --features asterisk
```

Wenn keine Asterisk-/Telefonie-Funktionen benötigt werden, kann je nach Projektstand auch ohne Feature gebaut werden:

```bash id="v3gzsl"
cargo build --release
```

Der Build erzeugt das Binary typischerweise unter:

```text id="dr6fit"
target/release/bluestation-bs
```

Prüfen:

```bash id="hl8spp"
ls -lh target/release/
```

---

## Konfigurationsverzeichnis anlegen

Zurück als administrativer Benutzer oder mit sudo:

```bash id="frx0eo"
sudo mkdir -p /opt/netcore-tetra/config
sudo chown -R netcore:netcore /opt/netcore-tetra/config
```

Die Konfiguration ablegen:

```bash id="jzshqr"
/opt/netcore-tetra/config/config.toml
```

Eine Fallback-Konfiguration ist empfehlenswert:

```bash id="z79yfr"
cp /opt/netcore-tetra/config/config.toml \
   /opt/netcore-tetra/config/config.toml.fallback
```

Die Fallback-Datei sollte einen bekannten funktionierenden Stand enthalten.

---

## Wichtige Konfigurationsbereiche

Eine vollständige `config.toml` besteht aus mehreren Abschnitten.

Wichtige Grundbereiche:

```text id="gmy83p"
config_version
stack_mode
phy_io
net_info
cell_info
dashboard
netcore_directory
asterisk
security
```

Minimal relevant für den Betrieb:

```toml id="gqq0zs"
config_version = "0.6"
stack_mode = "Bs"
```

---

## SDR-Konfiguration

Die SDR-Konfiguration befindet sich im Abschnitt:

```toml id="y0daxk"
[phy_io]
backend = "SoapySdr"

[phy_io.soapysdr]
tx_freq = 418000000
rx_freq = 408000000
```

Die Frequenzen müssen zur restlichen Zellkonfiguration passen.

Wichtig:

```text id="uzcie7"
tx_freq = Downlink-Frequenz der Basisstation
rx_freq = Uplink-Frequenz der Basisstation
```

Diese Werte müssen mit `cell_info` zusammenpassen.

---

## Zell- und Netzkonfiguration

Die Netzkennung wird unter `net_info` gesetzt:

```toml id="lzkvxl"
[net_info]
mcc = 901
mnc = 1510
```

Die Zellparameter befinden sich unter `cell_info`.

Beispiel:

```toml id="j2tgsb"
[cell_info]
freq_band = 4
main_carrier = 720
```

Diese Werte bestimmen zusammen mit Band, Trägernummer und Duplexabstand die tatsächlich verwendeten Frequenzen.

---

## Dashboard aktivieren

Das Dashboard wird über die Konfiguration aktiviert.

Beispiel:

```toml id="zdamjt"
[dashboard]
enabled = true
bind_addr = "0.0.0.0"
port = 8080
```

Nach dem Start ist das Dashboard dann erreichbar unter:

```text id="vaapul"
http://<HOST-IP>:8080
```

Je nach Konfiguration können Benutzername, Passwort, Session-Login oder weitere Optionen erforderlich sein.

---

## NetCore Directory konfigurieren

Die Basisstation kann das NetCore Directory verwenden, um Geräte, Gruppen, Statusmeldungen und Statusgruppen aufzulösen.

Beispiel:

```toml id="ai87fn"
[netcore_directory]
enabled = true
base_url = "http://127.0.0.1:8095"
timeout_ms = 2000
```

Wenn Directory und Basisstation auf demselben Host laufen, kann `127.0.0.1` genutzt werden.

Wenn das Directory auf einem anderen Host läuft:

```toml id="fof9s2"
[netcore_directory]
enabled = true
base_url = "http://10.0.1.22:8095"
timeout_ms = 2000
```

---

## NetCore Directory installieren

Verzeichnis anlegen:

```bash id="a429fm"
sudo mkdir -p /opt/netcore-tetra/directory
sudo chown -R netcore:netcore /opt/netcore-tetra/directory
```

Directory-Datei ablegen:

```text id="h5d5mk"
/opt/netcore-tetra/directory/netcore_directory_server.py
```

Start zum Test:

```bash id="euy472"
sudo -iu netcore

cd /opt/netcore-tetra/directory

python3 netcore_directory_server.py \
  --host 0.0.0.0 \
  --port 8095 \
  --db ./netcore_directory.db
```

Healthcheck:

```bash id="ce5emf"
curl -s http://127.0.0.1:8095/api/health | jq .
```

Erwartet:

```json id="xo28ny"
{
  "ok": true,
  "name": "NetCore Directory",
  "version": "...",
  "time": "..."
}
```

---

## Directory systemd-Service

Datei erstellen:

```bash id="c18ds7"
sudo nano /etc/systemd/system/netcore-directory.service
```

Beispiel:

```ini id="so01ym"
[Unit]
Description=NetCore Directory
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=netcore
Group=netcore
WorkingDirectory=/opt/netcore-tetra/directory
ExecStart=/usr/bin/python3 /opt/netcore-tetra/directory/netcore_directory_server.py --host 0.0.0.0 --port 8095 --db /opt/netcore-tetra/directory/netcore_directory.db
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

Aktivieren:

```bash id="s16k56"
sudo systemctl daemon-reload
sudo systemctl enable --now netcore-directory
```

Status prüfen:

```bash id="l8js0n"
sudo systemctl status netcore-directory
```

Logs:

```bash id="vrlbwa"
sudo journalctl -u netcore-directory -f
```

---

## Basisstation systemd-Service

Datei erstellen:

```bash id="j76elf"
sudo nano /etc/systemd/system/tetra.service
```

Beispiel:

```ini id="n2s7uc"
[Unit]
Description=NetCore-Tetra Basisstation
After=network-online.target netcore-directory.service
Wants=network-online.target
Requires=netcore-directory.service

[Service]
Type=simple
User=netcore
Group=netcore
WorkingDirectory=/opt/netcore-tetra/flowstation
Environment=FLOWSTATION_CONFIG=/opt/netcore-tetra/config/config.toml
Environment=NETCORE_DIRECTORY_ENABLED=true
Environment=NETCORE_DIRECTORY_URL=http://127.0.0.1:8095
Environment=NETCORE_DIRECTORY_TIMEOUT_MS=2000
ExecStart=/opt/netcore-tetra/flowstation/target/release/bluestation-bs /opt/netcore-tetra/config/config.toml
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
```

Aktivieren:

```bash id="e33h5u"
sudo systemctl daemon-reload
sudo systemctl enable --now tetra.service
```

Status prüfen:

```bash id="sgy5n0"
sudo systemctl status tetra.service
```

Logs:

```bash id="ft4v3f"
sudo journalctl -u tetra.service -f
```

---

## Startreihenfolge

Empfohlene Reihenfolge:

```text id="y4vrir"
1. Netzwerk
2. NetCore Directory
3. Basisstation
4. Dashboard
5. Funkgeräte
```

Bei systemd kann das Directory als Abhängigkeit der Basisstation definiert werden.

---

## Funktionstest

### Directory erreichbar?

```bash id="rss7cu"
curl -s http://127.0.0.1:8095/api/health | jq .
```

### Geräte abrufbar?

```bash id="bwni48"
curl -s http://127.0.0.1:8095/api/devices | jq .
```

### Statusmeldungen abrufbar?

```bash id="zllx7y"
curl -s http://127.0.0.1:8095/api/status | jq .
```

### Statusgruppen abrufbar?

```bash id="yp9zlu"
curl -s 'http://127.0.0.1:8095/api/status-group-members?issi=2020001' | jq .
```

### Basisstation läuft?

```bash id="ol51g5"
sudo systemctl status tetra.service
```

### Live-Logs ansehen

```bash id="f7hblr"
sudo journalctl -u tetra.service -f
```

---

## Typische Logfilter

Registrierung und Affiliation:

```bash id="ehb0lh"
sudo journalctl -u tetra.service -f | egrep "MmSubscriberUpdate|Register|Affiliate|subscriber"
```

Status und Home Mode Display:

```bash id="g40ads"
sudo journalctl -u tetra.service -f | egrep "SDS-STATUS|HomeModeDisplay|status-sync"
```

Directory:

```bash id="k9u8ch"
sudo journalctl -u tetra.service -f | egrep "NetCore Directory|Directory|status-sync"
```

SDS:

```bash id="rz8xrm"
sudo journalctl -u tetra.service -f | egrep "SDS|U-STATUS|D-SDS"
```

---

## Update

Zum Aktualisieren:

```bash id="n35xhz"
sudo -iu netcore
cd /opt/netcore-tetra/flowstation

git pull
cargo build --release --features asterisk
```

Danach Dienst neu starten:

```bash id="qo8e4m"
sudo systemctl restart tetra.service
```

Logs beobachten:

```bash id="x67d7w"
sudo journalctl -u tetra.service -f
```

---

## Backup

Wichtige Dateien:

```text id="p2mcfm"
/opt/netcore-tetra/config/config.toml
/opt/netcore-tetra/config/config.toml.fallback
/opt/netcore-tetra/directory/netcore_directory.db
/opt/netcore-tetra/directory/*.json
```

Einfaches Backup:

```bash id="g4kqi2"
sudo mkdir -p /opt/netcore-tetra/backups

sudo tar -czf /opt/netcore-tetra/backups/netcore-backup-$(date +%F-%H%M).tar.gz \
  /opt/netcore-tetra/config \
  /opt/netcore-tetra/directory
```

Directory zusätzlich als JSON exportieren:

```bash id="sq5tx2"
curl -s http://127.0.0.1:8095/api/export \
  -o /opt/netcore-tetra/backups/netcore-directory-export-$(date +%F-%H%M).json
```

---

## Deinstallation

Dienste stoppen:

```bash id="rf9dgd"
sudo systemctl stop tetra.service
sudo systemctl stop netcore-directory.service
```

Dienste deaktivieren:

```bash id="fqe4xx"
sudo systemctl disable tetra.service
sudo systemctl disable netcore-directory.service
```

Service-Dateien entfernen:

```bash id="u5re1p"
sudo rm /etc/systemd/system/tetra.service
sudo rm /etc/systemd/system/netcore-directory.service
sudo systemctl daemon-reload
```

Datenverzeichnis entfernen, falls wirklich gewünscht:

```bash id="mpv3gr"
sudo rm -rf /opt/netcore-tetra
```

> Achtung: Dadurch werden Konfiguration, Directory-Datenbank und Backups gelöscht, sofern sie dort liegen.

---

## Häufige Probleme nach der Installation

### SDR wird nicht gefunden

Prüfen:

```bash id="ub2p1e"
SoapySDRUtil --find
SoapySDRUtil --probe
```

Mögliche Ursachen:

* Treiber fehlt,
* Gerät nicht eingesteckt,
* USB-Rechte fehlen,
* Benutzer nicht in passender Gruppe,
* falscher SDR-String in der Config.

---

### Basisstation startet nicht

Prüfen:

```bash id="dsl7en"
sudo systemctl status tetra.service
sudo journalctl -u tetra.service -n 100
```

Häufige Ursachen:

* fehlerhafte `config.toml`,
* fehlende SDR-Hardware,
* falsche Frequenzparameter,
* fehlende Berechtigungen,
* Binary nicht vorhanden,
* falscher Pfad in der systemd-Unit.

---

### Directory wird nicht erreicht

Prüfen:

```bash id="f1hfi6"
curl -s http://127.0.0.1:8095/api/health | jq .
sudo systemctl status netcore-directory
```

Häufige Ursachen:

* Dienst läuft nicht,
* falsche IP in `[netcore_directory]`,
* Firewall,
* falscher Port,
* Directory läuft auf anderem Host.

---

### Dashboard zeigt keine Namen

Prüfen:

```bash id="vvhe3g"
curl -s http://127.0.0.1:8095/api/devices | jq .
```

Mögliche Ursachen:

* Gerät nicht im Directory eingetragen,
* ISSI stimmt nicht,
* Directory-Anbindung deaktiviert,
* Cache noch nicht aktualisiert,
* Dashboard noch nicht neu geladen.

---

## Nächste Seiten

Nach der Installation sind besonders relevant:

* [[Configuration]]
* [[Systemd-Service]]
* [[NetCore-Directory]]
* [[Status-Feedback]]
* [[Troubleshooting]]
