# NetCore Control Room

This directory contains the software that belongs to the external Control Room / Leitstelle side of NetCore-Tetra.

The radio base station remains responsible for RF/TETRA runtime. The Control Room side runs outside of the TBS, typically in an LXC/VM, and native operator clients connect to it.

## Layout

```text
system-backend/control-room/
  operator/   Native operator client / Leitstellenkonsole
```

The existing `netcore-control-room` core service exposes the API that this operator client consumes. Future native GUI code should also live below this directory, not in the base-station binaries.
