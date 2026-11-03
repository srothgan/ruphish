---
title: Use PostgreSQL
description: Connect a new Ruphish installation to PostgreSQL with a dedicated role and verified TLS.
sidebar:
  order: 28
---

Use PostgreSQL 17 or 18 with a writable UTF8 database and SCRAM password authentication. You need an existing server reachable from Ruphish and a dedicated login role.

PostgreSQL supports a fresh Ruphish installation. There is no Gophish PostgreSQL conversion or built-in transfer from SQLite or MySQL.

## Prepare the database

Ask your database administrator to create a UTF8 database and a dedicated login role. Grant that role `CONNECT` and `CREATE` on the database so Ruphish can create and own its `ruphish` schema and tables. The role does not need superuser, role-creation, or database-creation privileges.

For example, once the database and role both exist under the name `ruphish`:

```sql
GRANT CONNECT, CREATE ON DATABASE ruphish TO ruphish;
```

## Configure a native service

Replace the existing database fields in `config.json` with this fragment, keeping the listener and logging settings:

```json
{
  "db_name": "postgres",
  "db_path": "",
  "postgres": {
    "host": "postgres.example.internal",
    "database": "ruphish",
    "username": "ruphish",
    "password_env": "DATABASE_SECRET",
    "tls": "verify_identity"
  }
}
```

Replace the host, database, and username. Set `DATABASE_SECRET` to the role's password in the Ruphish process environment through your service manager or secret-management system. The JSON field names the variable rather than expanding a variable inside a string.

For a private certificate authority, add `"ca_path": "/etc/ruphish/postgres-ca.pem"` inside `postgres` and make that PEM file readable by Ruphish. Otherwise, system certificate trust is used. The server certificate must cover the configured hostname.

## Configure Docker Compose

Alternatively, add these entries under the existing `ruphish` service:

```yaml
environment:
  DB_NAME: "postgres"
  POSTGRES_HOST: "postgres.example.internal"
  POSTGRES_DATABASE: "ruphish"
  POSTGRES_USERNAME: "ruphish"
  POSTGRES_PASSWORD_ENV: "DATABASE_SECRET"
  DATABASE_SECRET: "${DATABASE_SECRET:?Set DATABASE_SECRET before starting Compose}"
  POSTGRES_TLS: "verify_identity"
```

Supply `DATABASE_SECRET` through your deployment's protected environment. For a private CA, mount the PEM file read-only and set `POSTGRES_CA_PATH` to its path inside the container. Apply the change with `docker compose up -d --wait`.

Selecting PostgreSQL replaces the image's SQLite connection settings, but does not transfer the existing data.

## Check the connection

Start or restart Ruphish and check its [logs and readiness](../health-and-logs.md). A new database receives the application schema automatically. If startup fails, check connectivity, credentials, permissions, and certificate trust before changing the database.

Remove MySQL settings and `db_sslca_path` from the configuration. Unset `PGOPTIONS`, `PGSSLCERT`, `PGSSLKEY`, and `PGSSLROOTCERT` in the application environment; Ruphish's own configuration controls these connection policies. Password files are not read.

## Connection options

All fields below belong inside `postgres`:

| Field | Default or requirement |
| --- | --- |
| `host`, `database`, `username` | Required nonempty strings. The host contains no scheme or port. |
| `port` | `5432`; nonzero. |
| `password` or `password_env` | Exactly one password source. |
| `tls` | `verify_identity`. `disabled` explicitly permits plaintext for a controlled local test connection and cannot include a CA. |
| `ca_path` | Optional PEM CA file. |
| `max_connections` | `10`; range `1–128`. |
| `connect_timeout_seconds` | `10`; range `1–300`. |
| `acquire_timeout_seconds` | `10`; range `1–300`. |
| `lock_timeout_seconds` | `10`; range `1–300`. |
| `statement_timeout_seconds` | `30`; range `1–300`. |

Use the typed settings above rather than a connection string. Unknown populated databases, altered migration history, and schema drift stop startup. PostgreSQL's transactional schema changes do not replace your own [backups](../back-up-and-restore.md); Ruphish does not create a separate PostgreSQL recovery copy for you.
