---
title: Back up and restore
description: Preserve database and configuration state, restore a separate copy, and check it before returning to service.
sidebar:
  order: 31
---

A usable backup contains your database and the settings needed to run it. Take backups regularly and before upgrades or storage changes. Keep copies outside the Ruphish host so a lost disk or deleted volume does not remove both the installation and its backup.

## What to keep

| Item | Why it matters |
| --- | --- |
| Database | Accounts, shared resources, company settings, campaigns, results, integration credentials, and queued work. |
| Configuration and deployment files | `config.json`, Compose files, service units, proxy configuration, and any environment settings needed to start the same installation. |
| Secret and certificate sources | Database passwords, custom certificate/private-key files, and private CA files that are not stored in the database. |
| Additional public files | Any files you supply under `static/endpoint/`. |
| Application version | The binary version or image digest used with the backup, so you can restore with the same release. |

Released binaries and Docker images include the browser interface. Restore a matching release; you do not need to rebuild web assets from source.

Backups can contain service credentials and personal data, including historical data retained from Gophish. Restrict access and apply your organization's retention rules to the backups as well as the live database.

## Back up SQLite in Docker

These commands use a POSIX shell, such as Bash on Linux, macOS, or WSL, from the installation folder containing `compose.yaml`. They apply to the documented SQLite volume at `/var/lib/ruphish`. Stop every process that writes to this database before copying it.

First, record the running image and stop the application:

```sh
mkdir -p backups
docker compose images ruphish
docker compose stop ruphish
```

Create an archive without starting Ruphish:

```sh
docker compose run --rm --no-deps --user 0 --entrypoint tar -v "${PWD}/backups:/backup" ruphish -czf /backup/ruphish-data.tar.gz -C /var/lib/ruphish .
```

This uses a temporary container with the service's existing volume and runs `tar` instead of the application. It includes the entire data directory, including any SQLite companion files. The fixed filename replaces an earlier archive with the same name; use a dated filename or move it into your backup storage before the next run.

Check that the archive can be read, then start the service again:

```sh
docker compose run --rm --no-deps --user 0 --entrypoint tar -v "${PWD}/backups:/backup:ro" ruphish -tzf /backup/ruphish-data.tar.gz
docker compose up -d --wait
```

The listing should include `ruphish.db`. If archive creation or listing fails, preserve the stopped data and correct the failure before relying on this backup. Save `compose.yaml`, any mounted configuration, and external secrets separately. The archive does not contain Caddy's separate certificate volumes or files outside `/var/lib/ruphish`. See [Docker's volume backup guidance](https://docs.docker.com/engine/storage/volumes/) for the underlying container-volume workflow.

## Back up SQLite in a native service

For the [Linux service setup](binary-service.md), run:

```sh
sudo systemctl stop ruphish
sudo tar -czf /var/backups/ruphish-data.tar.gz -C /var/lib/ruphish .
sudo chmod 0600 /var/backups/ruphish-data.tar.gz
sudo tar -tzf /var/backups/ruphish-data.tar.gz
sudo systemctl start ruphish
```

Run the commands in order and check each result before continuing. The example assumes `/var/backups` exists and the data directory is `/var/lib/ruphish`. Copy the configuration, service unit, and external secrets into your protected backup storage too.

For a foreground native installation on another platform, stop Ruphish with **Ctrl+C** and copy its [configuration and data directories](configuration.md#default-locations-and-explicit-configuration) with your normal backup tool. Copy a live SQLite database only with a SQLite-aware backup process; copying just the `.db` file while it is being written can miss committed data in its journal or WAL.

## Back up MySQL or PostgreSQL

Use your database provider's consistent backup and restore tools, including the application's schema and migration metadata. Coordinate backup timing with the person operating the database, and verify that the Ruphish service role can access the restored schema.

A Docker data-volume archive does not back up a remote MySQL or PostgreSQL database. Preserve the connection configuration and secrets separately from the database backup.

Ruphish's automatic SQLite pre-upgrade snapshots and MySQL conversion recovery tables are additional recovery points. They do not replace regular backups and do not include work performed afterward.

## Restore into a separate location

Start with a separate path, volume, or database so the original remains available. Use the same application release recorded with the backup. Keep the restored application isolated from real mail servers, reporting mailboxes, webhook receivers, and campaign traffic until verification is complete.

### Docker SQLite volume

Create a separate folder and save this as `compose.yaml`. Replace the image with the exact release tag or digest recorded for the backup; `latest` is shown only as the usual image name:

```yaml
services:
  ruphish:
    image: ghcr.io/srothgan/ruphish:latest
    volumes:
      - restored-data:/var/lib/ruphish
    network_mode: none
volumes:
  restored-data:
```

Put the trusted backup archive at `backups/ruphish-data.tar.gz` in this new folder. Restore it to a new volume with a distinct Compose project name:

```sh
docker compose -p ruphish-restore run --rm --no-deps --user 0 --entrypoint tar -v "${PWD}/backups:/backup:ro" ruphish -xzf /backup/ruphish-data.tar.gz -C /var/lib/ruphish
```

Use an empty destination volume. The command restores the archive's file ownership, including the image's service user `10001`. It does not start Ruphish or expose any ports. Keep the restore project name for subsequent commands so they continue to select this volume.

For a first startup check without network access, run:

```sh
docker compose -p ruphish-restore run --rm --no-deps ruphish --disable-mailer
```

Look for `RUPHISH_READY`, then press **Ctrl+C** to stop. Network-dependent integration checks may fail in this isolated container, which is expected. Do not treat this startup check alone as a complete restore verification.

### Native SQLite directory

For a Linux restore, extract to a new directory:

```sh
sudo install -d -o ruphish -g ruphish -m 0700 /var/lib/ruphish-restore
sudo tar -xzf /var/backups/ruphish-data.tar.gz -C /var/lib/ruphish-restore
sudo chown -R ruphish:ruphish /var/lib/ruphish-restore
```

Use a separate configuration whose database and writable paths point into that restored directory. Keep listeners on unused loopback ports and use an isolated host or network policy that blocks real integrations during testing. Do not start the ordinary production service against the restored copy while the original is still active.

### Server database

Restore to a new database using the provider's tooling. Create or restore the required service account permissions, then point a separate Ruphish configuration at that database. Do not overwrite the live database as a way to test a backup.

## Verify a restore

In a controlled environment with real outbound integrations blocked, check:

1. Ruphish starts with the expected version, configuration, and database, and readiness returns `200`.
2. An existing account can sign in.
3. Shared resources, recipient groups, campaigns, and results match the backup point.
4. Scheduled campaigns and unfinished delivery work are understood before sending is enabled.
5. Company settings, sending profiles, reporting mailboxes, webhooks, and campaign domains are present as expected.

For browser checks, give the isolated instance a local browser port while retaining the outbound restrictions. `--disable-mailer` stops campaign queue execution only; it does not stop all integrations. Restoring an old backup may make previously sent work appear pending again, so reconcile the backup point with actual delivery before returning it to service.

When the restored installation is ready to take over, stop the old instance, select the restored storage and matching configuration, restore the intended network access, and verify readiness and sign-in again. Keep only the intended instance serving campaign traffic and sending mail.
