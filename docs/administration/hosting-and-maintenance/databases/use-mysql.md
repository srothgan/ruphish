---
title: Use MySQL
description: Connect Ruphish to a MySQL database with a dedicated account and verified TLS.
sidebar:
  order: 27
---

Use MySQL 8.4 or 9.7 LTS with InnoDB. You need an existing server reachable from Ruphish and a dedicated database account. MariaDB and other MySQL release lines are not accepted substitutes.

For an existing Gophish database, follow [Migration from Gophish](../../../migration/migrate-the-database.md) before starting Ruphish against it. The steps below are for a new installation.

## Prepare the database

Have your database administrator create the database:

```sql
CREATE DATABASE ruphish CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin;
```

Create a dedicated account that can connect from the Ruphish host. Grant it database-scoped `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`, `DROP`, `INDEX`, `CREATE VIEW`, and `SHOW VIEW` on `ruphish.*`. Ruphish needs these permissions for normal operations and automatic schema updates. It does not need server-wide privileges or permission to create databases or accounts.

## Configure a native service

Replace the existing database fields in `config.json` with the following, keeping your listener and logging settings:

```json
{
  "db_name": "mysql",
  "db_path": "",
  "mysql": {
    "host": "mysql.example.internal",
    "database": "ruphish",
    "username": "ruphish",
    "password_env": "DATABASE_SECRET",
    "tls": "verify_identity"
  }
}
```

Replace the host, database, and username. Set `DATABASE_SECRET` in the Ruphish process environment through your service manager or secret-management system. It must contain the account's password; the JSON field names the variable, not its value.

If the server uses a private certificate authority, add `"ca_path": "/etc/ruphish/mysql-ca.pem"` inside `mysql` and make that PEM file readable by Ruphish. Otherwise, system certificate trust is used. The server certificate must cover the configured hostname.

## Configure Docker Compose

Alternatively, add these entries under the existing `ruphish` service:

```yaml
environment:
  DB_NAME: "mysql"
  MYSQL_HOST: "mysql.example.internal"
  MYSQL_DATABASE: "ruphish"
  MYSQL_USERNAME: "ruphish"
  MYSQL_PASSWORD_ENV: "DATABASE_SECRET"
  DATABASE_SECRET: "${DATABASE_SECRET:?Set DATABASE_SECRET before starting Compose}"
  MYSQL_TLS: "verify_identity"
```

Supply `DATABASE_SECRET` to Compose through your deployment's protected environment. The example passes it into the container for Ruphish to read. Keep secrets out of shared Compose files and diagnostic output. For a private CA, also mount the PEM file read-only and set `MYSQL_CA_PATH` to its path inside the container.

Selecting `DB_NAME: "mysql"` replaces the image's SQLite connection settings. It does not import the existing SQLite data. Apply the change with `docker compose up -d --wait`.

## Check the connection

Start or restart Ruphish and inspect its [logs and readiness](../health-and-logs.md). A new database receives the application schema automatically. Sign in and check that you selected the intended installation.

If startup fails, check DNS and port access from the application host, the password variable, database permissions, and certificate trust. Do not disable TLS to work around a hostname or CA error on a hosted database.

## Connection options

All fields below belong inside `mysql`:

| Field | Default or requirement |
| --- | --- |
| `host`, `database`, `username` | Required nonempty strings. The host contains no scheme or port. |
| `port` | `3306`; nonzero. |
| `password` or `password_env` | Exactly one password source. Prefer `password_env` when the service environment manages secrets. |
| `tls` | `verify_identity`. `disabled` explicitly permits plaintext for a controlled local test connection and cannot include a CA. |
| `ca_path` | Optional PEM CA file. |
| `max_connections` | `10`; range `1–128`. |
| `connect_timeout_seconds` | `10`; range `1–300`. |
| `acquire_timeout_seconds` | `10`; range `1–300`. |
| `lock_timeout_seconds` | `10`; range `1–300`. |
| `legacy_timezone` | Optional fixed UTC offset for recognized Gophish histories from before UTC timestamps. Not needed for a fresh database. |

Do not combine this object with a legacy MySQL connection string or `db_sslca_path`. MySQL schema changes are not generally transactional, so an interrupted upgrade may require [recovery from backup](../back-up-and-restore.md).
