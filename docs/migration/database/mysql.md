---
title: Migrate MySQL
description: Convert a recognized Gophish MySQL database with explicit server, timestamp, and recovery requirements.
sidebar:
  order: 42
prev: {"label": "Migration preparation", "link": "/docs/migration/from-gophish/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Use this procedure for a recognized Gophish MySQL database. Complete [preparation](../from-gophish.md#prepare) first. Ruphish accepts MySQL **8.4 or 9.7 LTS**, using InnoDB. MariaDB and other MySQL release lines are rejected. Moving an older installation to a supported MySQL server is a separate prerequisite; verify the restored Gophish database there before converting it.

## 1. Back up and restore a rehearsal database

Stop Gophish and all other writers. The following example uses MySQL client tools and prompts for the backup account password. Replace the host, user, and source database name:

```sh
mysqldump --host=mysql.example.internal --user=backup_operator --password --single-transaction --no-tablespaces --set-gtid-purged=OFF --result-file=gophish-backup.sql gophish
```

Use your deployment's required TLS options when connecting. Protect the dump: it contains API keys, service credentials, and historical participant data. Confirm that the command succeeded and keep the dump outside the working database.

Restore into a separate database on an isolated supported server. Do not restore over the source. In the MySQL client connected as an account allowed to create the rehearsal database:

```sql
CREATE DATABASE ruphish_rehearsal CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin;
USE ruphish_rehearsal;
SOURCE /path/to/gophish-backup.sql;
```

Replace the dump path. Check restoration errors, expected tables, migration history, and representative row counts before starting Ruphish. The dump example omits `--databases` so it does not select or recreate the original database during restore.

Give the Ruphish database account database-scoped `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`, `DROP`, `INDEX`, `CREATE VIEW`, and `SHOW VIEW`. See [MySQL configuration](../../administration/hosting-and-maintenance/databases/mysql.md). Keep the source schema intact: unrecognized additional tables, views, triggers, foreign keys, or altered migration history can cause rejection.

## 2. Resolve historical timestamps

For recognized histories from before Gophish stored UTC timestamps, startup requires `mysql.legacy_timezone`. Supply the fixed UTC offset used by those stored dates, such as `"+00:00"` only when they were actually UTC. This is not the current timezone of the Ruphish server or a browser preference.

If the source used daylight-saving changes, establish the meaning of the historical dates before choosing one offset. The option accepts a fixed offset, not an IANA timezone with date-dependent rules. Do not guess an offset merely to get past startup. Later UTC histories do not need this option.

## 3. Configure Ruphish

Install the released binary using [Install Ruphish](../../getting-started/installation.mdx). Save the following as `config.json` in a dedicated migration folder:

```json
{
  "admin_server": {"listen_url": "127.0.0.1:3333", "use_tls": false},
  "phish_server": {"listen_url": "127.0.0.1:8081", "use_tls": false},
  "db_name": "mysql",
  "db_path": "",
  "mysql": {
    "host": "mysql-rehearsal.example.internal",
    "database": "ruphish_rehearsal",
    "username": "ruphish",
    "password_env": "DATABASE_SECRET",
    "tls": "verify_identity"
  },
  "logging": {"filename": "", "level": "info"}
}
```

Replace the host, database, and user. Supply `DATABASE_SECRET` through the process environment. For a private CA, add `ca_path` inside `mysql` and provide its readable PEM file. Add `legacy_timezone` only when required by the source history. Do not combine the typed `mysql` object with a legacy connection string or `db_sslca_path`.

Allow the rehearsal host to reach only its copied MySQL database and the local tools needed for inspection. Keep production SMTP, IMAP, webhook, and domain-verification destinations blocked. Then run:

```sh
ruphish --config ./config.json --disable-mailer
```

### Docker alternative

The same conversion can run in the released image. Save this as `compose.yaml`, replace the MySQL connection values, and supply `DATABASE_SECRET` to Compose through your protected environment:

```yaml
services:
  ruphish:
    image: ghcr.io/srothgan/ruphish:latest
    command: ["--disable-mailer"]
    stop_grace_period: 30s
    ports:
      - "127.0.0.1:3333:3333"
      - "127.0.0.1:8081:8081"
    environment:
      DB_NAME: "mysql"
      MYSQL_HOST: "mysql-rehearsal.example.internal"
      MYSQL_DATABASE: "ruphish_rehearsal"
      MYSQL_USERNAME: "ruphish"
      MYSQL_PASSWORD_ENV: "DATABASE_SECRET"
      DATABASE_SECRET: "${DATABASE_SECRET:?Set DATABASE_SECRET before starting Compose}"
      MYSQL_TLS: "verify_identity"
    volumes:
      - migration-data:/var/lib/ruphish
volumes:
  migration-data:
```

This Compose file does not itself isolate outbound traffic: configure the host/network firewall to allow the rehearsal MySQL connection while blocking production integration destinations before starting it. For a private CA, mount the PEM file and set `MYSQL_CA_PATH`. For pre-UTC histories, add the verified `MYSQL_LEGACY_TIMEZONE` offset.

```sh
docker compose pull ruphish
docker compose up -d --wait ruphish
docker compose logs --tail=100 ruphish
```

## 4. Understand conversion and restart behavior

Startup takes a database-scoped coordination lock, recognizes the source schema/history, and validates source data. It creates `_ruphish_upgrade` to record progress, copies into `rp_stage_*` tables, and publishes the converted tables with an atomic InnoDB rename. The old tables remain as `rp_original_*`; they are recovery data, not active Ruphish records. Application schema v1 and its history are validated before ordinary operation starts.

An interrupted recognized conversion can resume from its journal with the same release. Startup can rebuild an incomplete staging copy or publish a completed copy. This does not make arbitrary schema edits recoverable. Do not manually remove the journal, staging tables, or retained originals to bypass a failure.

Keep enough storage for the source tables, converted tables, indexes, and your independent backup. Retained originals contain the old secrets and captured data even where active Ruphish records have been sanitized.

## Continue with your client

Before the final cutover, follow the path you will use:

| Client | Next step |
| --- | --- |
| Ruphish interface | [Find your tasks and review migrated work](../using-the-interface.md). |
| Your own UI, script, or service | [Adapt the endpoints your client uses](../api/index.md), then test them against this rehearsal database. |

## 5. Verify and cut over

Wait for `RUPHISH_READY`. Open `http://127.0.0.1:3333` with an existing account, or inspect migrated resources with an adapted API client. Complete [verification](../verification-and-rollback.md), including timestamp checks and pending deliveries.

If startup rejects the database, preserve the error and current schema. Check the server version, grants, TLS, recognized history, and required legacy timezone. Restore the backup into a new rehearsal database if another clean attempt is needed; do not treat a new empty database as a successful migration.

After rehearsal, stop Gophish, take a fresh backup, and repeat the verified conversion using the same Ruphish release. Apply production routing and HTTPS, resolve imported delivery/integration issues, then deliberately enable workers and external connections. Record the tested Docker digest if using `latest`.

For rollback, stop Ruphish and restore the independent pre-conversion backup into a separate database for the matching Gophish installation. Retained original tables assist diagnosis and recovery, but there is no operator command here that automatically swaps them back. Follow [Verification and rollback](../verification-and-rollback.md).
