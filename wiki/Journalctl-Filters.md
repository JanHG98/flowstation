# Journalctl-Filter

## Live-Log

```bash
sudo journalctl -u tetra.service -f
```

## Aktueller Boot

```bash
sudo journalctl -u tetra.service -b --no-pager
```

## Registrierung und Roaming

```bash
sudo journalctl -u tetra.service -f | \
  grep --line-buffered -iE 'register|location|roaming|attract|affiliate'
```

## Rufe, Carrier und Timeslots

```bash
sudo journalctl -u tetra.service -f | \
  grep --line-buffered -iE 'call|setup|connect|release|floor|carrier|timeslot|TCH'
```

## SDS, Status und HMD

```bash
sudo journalctl -u tetra.service -f | \
  grep --line-buffered -iE 'SDS|U-STATUS|status|HMD|home.mode|emergency'
```

## Audio, Recorder und TTS

```bash
sudo journalctl -u tetra.service -f | \
  grep --line-buffered -iE 'AudioPlayer|record|recording|TTS|Piper|ffmpeg|archive|NFS'
```

## SDR und Timing

```bash
sudo journalctl -u tetra.service -f | \
  grep --line-buffered -iE 'Soapy|SDR|sample|buffer|underflow|overflow|timing|passband|PHY'
```

## Directory und Leitstelle

```bash
sudo journalctl -u tetra.service -f | \
  grep --line-buffered -iE 'directory|control.room|websocket|connect|auth|TLS|broken.pipe'
```

## Zeitfenster exportieren

```bash
sudo journalctl -u tetra.service \
  --since '2026-07-21 12:00:00' \
  --until '2026-07-21 12:15:00' \
  -o short-iso --no-pager > tetra-zeitfenster.log
```

Vor Weitergabe Zugangsdaten, Nachrichtentexte, ISSIs und Standorte prüfen und bei Bedarf schwärzen.
