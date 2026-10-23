---
title: Use MySQL
description: Configure verified MySQL connections and understand provider-specific upgrade requirements.
sidebar:
  order: 3
---

Provision an InnoDB/UTF-8 database and a dedicated service account. Grant database-scoped `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`, `DROP`, `INDEX`, `CREATE VIEW`, and `SHOW VIEW` for runtime operations and automatic migrations. Observation migration v5 introduces the `event_history` compatibility view and requires the two view privileges before upgrade. Server-wide privileges, triggers, and database/account creation permissions are not required.

Replace the SQLite fields with this configuration fragment:

```json
{
  "db_name": "mysql",
  "db_path": "",
  "mysql": {
    "host": "mysql.example.internal",
    "database": "ruphish",
    "username": "ruphish",
    "password_env": "RUPHISH_MYSQL_PASSWORD",
    "tls": "verify_identity",
    "ca_path": "/path/to/mysql-ca.pem"
  }
}
```

Keep listener settings alongside it. Set the named password environment variable privately before startup. Replace the dummy `ca_path` with an actual file path, or omit it when the server uses a publicly trusted certificate. This JSON field does not expand environment variables; `password_env` explicitly names an environment variable for the password.

| Typed field | Default/constraint |
| --- | --- |
| `host`, `database`, `username` | Required nonempty endpoint/database/account names; host has no scheme or port |
| `port` | 3306; must be nonzero |
| `password`, `password_env` | Exactly one source |
| `tls` | `verify_identity`; explicit `disabled` is for controlled plaintext test connections, with no CA |
| `ca_path` | Optional PEM CA file |
| `max_connections` | 10; range 1–128 |
| `connect_timeout_seconds`, `acquire_timeout_seconds`, `lock_timeout_seconds` | 10 each; range 1–300 |
| `legacy_timezone` | Optional fixed offset such as `+02:00`, required for recognized pre-UTC Gophish histories |

Typed settings cannot be combined with a legacy DSN or `db_sslca_path`. Supported legacy TCP DSNs are a migration convenience, not a promise to accept every Go driver option. Custom migration directories are unsupported.

## Upgrade and recovery

Stop application writers and take an operator-managed backup first. Recognized legacy upgrades copy into staging tables, verify data, and publish with an atomic rename. Original `rp_original_*` tables and `_ruphish_upgrade` form the recovery record; do not rename/delete them independently.

Pre-UTC histories require the original fixed offset. The application does not infer it from today's server time zone; mixed historical offsets need assessment before migration. MySQL DDL is not generally transactional, so an interrupted ordinary schema migration can require recovery from backup. See [migration verification and rollback](../../migration/verification-and-rollback.md).
