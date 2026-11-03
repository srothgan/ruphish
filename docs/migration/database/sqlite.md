---
title: Migrate SQLite
description: Convert a copied Gophish SQLite database with a released Ruphish binary or Docker image.
sidebar:
  order: 41
prev: {"label": "Migration preparation", "link": "/docs/migration/from-gophish/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Use this procedure for a recognized Gophish SQLite database. Complete [preparation](../from-gophish.md#prepare) first. You need the SQLite command-line tool for the backup example and enough free space for the working database, its recovery snapshot, and temporary conversion work.

## Historical dates before UTC

SQLite histories before `20170827141312` convert stored dates using SQLite's local-to-UTC conversion in the Ruphish process environment. Run that conversion with the timezone interpretation that matches the original stored dates, and verify representative dates across any daylight-saving transitions. Do not assume a Docker container inherits the original host's timezone. For these histories, use a correctly configured native rehearsal environment unless you have explicitly configured and verified the container's timezone behavior. The MySQL `legacy_timezone` option does not apply to SQLite.

## 1. Make a consistent copy

Stop Gophish. In a new migration folder, run the following, replacing `/path/to/gophish.db` with the actual existing database file:

```sh
sqlite3 -readonly /path/to/gophish.db ".backup 'gophish-backup.db'"
sqlite3 -readonly gophish-backup.db "PRAGMA integrity_check;"
```

The integrity check must return `ok`. The backup operation includes committed WAL data; copying only a live `.db` file can omit committed changes. Keep `gophish-backup.db` as the untouched recovery copy.

Choose either the native or Docker path below. Each creates a separate working copy from this backup.

## 2A. Start with the native binary

Install the binary using the [normal installer](../../getting-started/installation.mdx). From the migration folder, copy the backup to the filename Ruphish will open.

On Linux or macOS:

```sh
mkdir data
cp gophish-backup.db data/ruphish.db
```

In PowerShell:

```powershell
New-Item -ItemType Directory -Path data
Copy-Item -LiteralPath gophish-backup.db -Destination data/ruphish.db
```

Save the following as `config.json` in the migration folder:

```json
{
  "admin_server": {
    "listen_url": "127.0.0.1:3333",
    "use_tls": false
  },
  "phish_server": {
    "listen_url": "127.0.0.1:8081",
    "use_tls": false
  },
  "db_name": "sqlite3",
  "db_path": "data/ruphish.db",
  "logging": {
    "filename": "",
    "level": "info"
  }
}
```

Apply the rehearsal host's outbound restrictions before starting. Run from this same folder because paths inside an explicitly selected configuration are relative to the working directory:

```sh
ruphish --config ./config.json --disable-mailer
```

This example binds both listeners to the local computer and uses HTTP. It is for rehearsal, not a replacement for the production hostname and TLS configuration. Stop the process with **Ctrl+C** when finished.

## 2B. Start with Docker Compose

This path copies the backup into a named volume and isolates the application from external networks. Save this as `compose.yaml` beside `gophish-backup.db`:

```yaml
services:
  ruphish:
    image: ghcr.io/srothgan/ruphish:latest
    command: ["--disable-mailer"]
    stop_grace_period: 30s
    ports:
      - "127.0.0.1:3333:3333"
      - "127.0.0.1:8081:8081"
    volumes:
      - migration-data:/var/lib/ruphish
    networks:
      - rehearsal
networks:
  rehearsal:
    internal: true
volumes:
  migration-data:
```

Download the image and create the stopped container:

```sh
docker compose pull ruphish
docker compose create ruphish
docker compose cp gophish-backup.db ruphish:/var/lib/ruphish/ruphish.db
```

The image runs as user/group `10001`. Give that user ownership of the copied file before the first start:

```sh
docker compose run --rm --no-deps --user 0 --entrypoint chown ruphish 10001:10001 /var/lib/ruphish/ruphish.db
docker compose up -d --wait ruphish
docker compose logs --tail=100 ruphish
```

Use a new Compose project/folder so this rehearsal cannot select an existing production volume. Do not run the copy command over a database that has already been opened by Ruphish.

## 3. Confirm conversion

Startup checks database integrity, the recorded Gophish migration history, and table structure. Before converting a recognized database, it creates and verifies `ruphish-pre-upgrade-<random>.sqlite3` in the same directory. It then converts the working database to application schema v1 inside a transaction and validates the result before committing.

For the native example, the recovery snapshot is in `data/`. For Docker, it is inside the `migration-data` volume. Keep your independent backup even when this snapshot exists.

Wait for `RUPHISH_READY`, then open `http://127.0.0.1:3333` and sign in with an existing account. An API client can instead check the migrated resources with its existing API key after adapting the affected calls. Check [readiness](../../administration/hosting-and-maintenance/monitoring.md#check-readiness) and complete the [migration verification](../verification-and-rollback.md).

If the installation appears empty or creates a new administrator unexpectedly, stop and verify the selected database path and Docker volume. A missing database path can create a fresh installation; it is not evidence that the source was migrated.

## 4. Handle a failed start

An unknown schema/history or failed integrity check is a stop condition. Keep the original backup, working file, snapshot if created, and startup error. Do not delete migration history or alter tables to force recognition.

If conversion fails before commit, its transaction is rolled back; startup does not continue into ordinary operation. Inspect the error before retrying the same release. Restore from the independent backup when a fresh attempt is needed, keeping the failed copy separately for diagnosis.

## Continue with your client

Before the final cutover, follow the path you will use:

| Client | Next step |
| --- | --- |
| Ruphish interface | [Find your tasks and review migrated work](../using-the-interface.md). |
| Your own UI, script, or service | [Adapt the endpoints your client uses](../api/index.md), then test them against this rehearsal database. |

## 5. Cut over

After rehearsal succeeds, stop Gophish again, take a fresh backup, and repeat the procedure with a fresh working database. Use the same tested Ruphish release; record the Docker image digest so a later pull of `latest` does not silently change the tested version.

Apply the production [network and HTTPS configuration](../../administration/hosting-and-maintenance/network-and-https.md), preserving the origins used in existing messages. Resolve imported delivery and integration issues before removing `--disable-mailer` or allowing outbound access. For Docker, recreate the service after changing its command or network configuration. Follow [verification and rollback](../verification-and-rollback.md) before reopening the installation to users.
