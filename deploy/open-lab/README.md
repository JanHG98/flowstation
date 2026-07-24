# NetCore Open-Lab LXC Deployment

This directory is the final cross-LXC integration layer for the current lab phase. It does not turn the management plane into a production system: every backend WebUI remains reachable without login, token or TLS and therefore belongs on an isolated management VLAN only.

## Offline workflow

```bash
cp deploy/open-lab/inventory.example.toml deploy/open-lab/inventory.toml
$EDITOR deploy/open-lab/inventory.toml
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml validate
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml plan
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml render
```

`render` rewrites service-to-service URLs by management port, creates the service catalog, `/etc/hosts` example, CSV port list and Graphviz dependency graph.

## Deployment

```bash
# Shows every scp/ssh action but changes nothing.
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml apply --dry-run

# Explicit real deployment after reviewing the plan and rendered configs.
python3 deploy/open-lab/netcore-deploy.py --inventory deploy/open-lab/inventory.toml apply
```

The deployer creates a deterministic source archive without PDFs, `.git`, `target`, caches or Node modules. Each LXC builds its own binary through the service's existing installer, receives its rendered config and is restarted in dependency order.

## Requirements

- Debian 13 or compatible LXC with systemd,
- Rust toolchain and C build dependencies on each build LXC,
- root SSH key access from the deployment host,
- isolated management network,
- `/dev/net/tun` passthrough for the IP Gateway,
- NFS mount prepared separately for Recorder/Media Library when archive features are used.

The tool intentionally does not store passwords, tokens, TLS keys, KMF master material or connector secrets.
