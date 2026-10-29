---
title: Use PostgreSQL
description: Configure a dedicated PostgreSQL database with verified TLS and owned migrations.
sidebar:
  order: 4
---

Provision a writable UTF8 database with SCRAM authentication and a dedicated login role. The role needs `CONNECT` and `CREATE` on that database so it can create and own the `ruphish` schema and its tables. It does not need superuser, role creation, or database creation privileges.

```json
{
  "db_name": "postgres",
  "db_path": "",
  "postgres": {
    "host": "postgres.example.internal",
    "database": "ruphish",
    "username": "ruphish",
    "password_env": "RUPHISH_POSTGRES_PASSWORD",
    "tls": "verify_identity",
    "ca_path": "/path/to/postgres-ca.pem"
  }
}
```

Keep the listener settings alongside this fragment. Supply the named password environment variable privately. Replace the dummy `ca_path` with an actual file path, or omit it for a publicly trusted certificate. This JSON field does not expand environment variables; `password_env` explicitly names an environment variable for the password.

| Typed field | Default/constraint |
| --- | --- |
| `host`, `database`, `username` | Required nonempty endpoint/database/account names; host has no scheme or port |
| `port` | 5432; must be nonzero |
| `password`, `password_env` | Exactly one source |
| `tls` | `verify_identity`; `disabled` must be explicit and cannot include a CA |
| `ca_path` | Optional PEM CA file |
| `max_connections` | 10; range 1–128 |
| `connect_timeout_seconds`, `acquire_timeout_seconds`, `lock_timeout_seconds` | 10 each; range 1–300 |
| `statement_timeout_seconds` | 30; range 1–300 |

Use typed settings, not a DSN. Remove MySQL settings, legacy `db_sslca_path`, and custom migration paths. Unset `PGOPTIONS`, `PGSSLCERT`, `PGSSLKEY`, and `PGSSLROOTCERT`; application configuration owns session/TLS policy. Password files are not read.

Fresh PostgreSQL installations initialize application schema v1 directly. There is no historical Gophish PostgreSQL upgrade or cross-engine importer. Unknown populated databases, modified migration history, and schema drift are rejected. Transactional DDL helps recover failed migrations but does not replace operator backups; PostgreSQL does not generate a recovery-copy path for you.
