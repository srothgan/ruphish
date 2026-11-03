---
title: Run with Docker Compose
description: Run Ruphish in the background with persistent data, automatic restarts, and bounded logs.
sidebar:
  order: 21
---

Use Docker Compose to keep Ruphish running in the background and preserve its data when you replace the container. You need Docker with Linux containers and Docker Compose. The image supports Linux x64 and ARM64, including through Docker Desktop on Windows and macOS.

If you followed [Install Ruphish](../../getting-started/installation.mdx), keep using that installation folder and its existing volume. The example below adds log retention to the same setup.

## Create the service

For a new installation, create a folder and open a terminal there:

```sh
mkdir ruphish
cd ruphish
```

Save this as `compose.yaml`:

```yaml
services:
  ruphish:
    image: ghcr.io/srothgan/ruphish:latest
    restart: unless-stopped
    stop_grace_period: 30s
    ports:
      - "127.0.0.1:3333:3333"
      - "127.0.0.1:8081:8081"
    volumes:
      - ruphish-data:/var/lib/ruphish
    logging:
      driver: local
      options:
        max-size: "10m"
        max-file: "5"
volumes:
  ruphish-data:
```

Start it from this folder:

```sh
docker compose pull
docker compose up -d --wait
docker compose ps
docker compose logs --tail=100 ruphish
```

Compose waits for the image's readiness check. The service should become healthy, and its startup output includes `RUPHISH_READY`. On a fresh installation, the logs also include the temporary password for username `admin`.

Open **http://127.0.0.1:3333** on the host and complete the first password change. The campaign listener is **http://127.0.0.1:8081**. These published ports accept connections only from the host computer; follow [Network access and HTTPS](network-and-https.md) before sharing the installation with other devices.

## Understand what persists

The `ruphish-data` volume stores SQLite at `/var/lib/ruphish/ruphish.db`. Compose normally prefixes the volume name with the project name, which comes from the installation folder. Keep using the same folder or explicit Compose project name so future commands select the same data.

The image supplies `/etc/ruphish/config.json`. It includes both listeners and SQLite defaults. Settings saved in the browser are in the database; process configuration comes from that JSON file and any environment overrides. The volume is persistent storage, not a backup.

The image runs as user and group `10001`. If you replace the named volume with a host-directory mount, that directory must be writable by this user. Mounted configuration and certificate files must be readable by it.

## Change configuration

Add environment settings under the existing `ruphish` service. For example:

```yaml
environment:
  LOGGING_LEVEL: "info"
  CONTACT_ADDRESS: "security@example.com"
```

Replace the example contact address with your team's address, then apply the change:

```sh
docker compose up -d --wait
```

Compose recreates the container when its configuration changes and keeps the data volume. `docker compose restart` alone does not apply changed Compose environment values. See the [configuration reference](configuration.md) for field names and database settings.

To supply a complete JSON configuration instead, create the file first and add `./config.json:/etc/ruphish/config.json:ro` to the service's `volumes`. Use container paths inside that file. Mount any files it references as well.

## Stop, start, and inspect

| Task | Command from the installation folder |
| --- | --- |
| Stop gracefully | `docker compose stop ruphish` |
| Start again | `docker compose up -d --wait` |
| Restart with the same configuration | `docker compose restart ruphish` |
| Follow recent output | `docker compose logs --tail=100 --follow ruphish` |
| Check status | `docker compose ps` |

Press **Ctrl+C** to stop following logs; the service continues running. Stopping Ruphish makes its website and campaign links unavailable until it starts again. Unfinished delivery work can resume after startup.

`docker compose down` removes the containers and network but retains named volumes. **Adding `--volumes` deletes those volumes and their data.** Use [Back up and restore](backup-and-restore.md) before any storage changes and [Upgrade Ruphish](upgrades.md) before replacing the image with a newer release.

Docker's restart policy handles process exits; an unhealthy status alone does not restart the container. See [Docker restart policies](https://docs.docker.com/engine/containers/start-containers-automatically/) and [health monitoring](monitoring.md).
