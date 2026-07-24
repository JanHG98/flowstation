# LXC-Deployment

Empfohlen: Debian-LXC mit 2 vCPU, 2 GiB RAM und ausreichend lokalem Storage.

```bash
sudo apt install build-essential pkg-config ffmpeg
sudo system-backend/media-library/install/install.sh
```

Anschließend:

```bash
sudo nano /etc/netcore/media-library.toml
sudo systemctl restart netcore-media-library
curl http://127.0.0.1:8230/health/ready
```

Für Archivierung wird das NFS-Share außerhalb des Dienstes nach `/mnt/nfs-share` gemountet. Der Dienst benötigt keinen privilegierten Container und keinen Zugriff auf `/dev`.
