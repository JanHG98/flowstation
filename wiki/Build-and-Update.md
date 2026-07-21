# Build und Update

## Grundsatz

Bei dieser Basisstation werden alte Build-Artefakte grundsätzlich entfernt. Gerade bei geänderten Features, nativen Bibliotheken oder mehreren Branches sind inkrementelle Altlasten eine unnötige Fehlerquelle.

## Sicheres Update aus Git

```bash
sudo systemctl stop tetra.service
cd ~/netcore

git status
git branch --show-current
git fetch --all --prune
git pull --ff-only
```

Wenn `git pull --ff-only` abbricht, nicht erzwingen. Erst lokale Änderungen prüfen und sauber zusammenführen.

## Vollständiger Clean-Build

```bash
cd ~/netcore
cargo clean
rm -rf target
cargo build --release -p bluestation-bs
```

Optional zusätzlich die Leitstellenwerkzeuge bauen:

```bash
cargo build --release -p netcore-control-room
cargo build --release -p netcore-control-room-operator
```

## Installation der neuen Binärdatei

Bei direktem Start aus dem Repository genügt der Neustart. Bei einer separaten Installation nach `/usr/local/bin`:

```bash
sudo install -m 0755 \
  target/release/bluestation-bs \
  /usr/local/bin/bluestation-bs
```

Anschließend:

```bash
sudo systemctl start tetra.service
sudo systemctl status tetra.service --no-pager
sudo journalctl -u tetra.service -n 200 --no-pager
```

## Update über das Dashboard

Das Dashboard kann einen explizit ausgelösten Quellcode-Updatevorgang anstoßen. Dafür muss es ein gültiges Git-Arbeitsverzeichnis finden. Falls die automatische Erkennung nicht passt, kann in `[dashboard]` ein `source_dir` gesetzt werden.

Vor einem Dashboard-Update gelten dieselben Regeln:

- Konfiguration und Fallback sichern.
- Lokale, nicht eingecheckte Änderungen vermeiden.
- Genügend Speicherplatz für den Build bereitstellen.
- Nach dem Neustart Logs und Versionsanzeige kontrollieren.

## Branchwechsel

```bash
sudo systemctl stop tetra.service
cd ~/netcore
git status
git fetch --all --prune
git switch <BRANCH>
git pull --ff-only
cargo clean
rm -rf target
cargo build --release -p bluestation-bs
sudo systemctl start tetra.service
```

## Rollback

Am saubersten ist ein bekannter Commit oder Tag:

```bash
sudo systemctl stop tetra.service
cd ~/netcore
git switch --detach <COMMIT-ODER-TAG>
cargo clean
rm -rf target
cargo build --release -p bluestation-bs
sudo systemctl start tetra.service
```

Für den dauerhaften Betrieb danach einen passenden Branch anlegen oder auf den vorherigen Branch zurückwechseln. Ein Detached-HEAD-Zustand sollte nicht unbemerkt zum normalen Arbeitsstand werden.
