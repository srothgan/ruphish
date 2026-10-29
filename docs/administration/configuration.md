---
title: Configuration reference
description: Configure listeners, databases, domain verification, logging, and startup secrets.
sidebar:
  order: 2
---

Ruphish reads JSON. Without `--config`, it reads the OS-specific `config.json` below and creates it with SQLite defaults on first startup. Existing files are never rewritten automatically. Changes take effect at restart; there is no hot-reload contract.

## Default locations and explicit configuration

| OS | Configuration | SQLite and generated certificates | Logs |
| --- | --- | --- | --- |
| Windows | `%LOCALAPPDATA%\ruphish\config.json` | `%LOCALAPPDATA%\ruphish\data\` | `%LOCALAPPDATA%\ruphish\logs\` |
| Linux | `$XDG_CONFIG_HOME/ruphish/config.json`, otherwise `~/.config/ruphish/config.json` | `$XDG_DATA_HOME/ruphish/`, otherwise `~/.local/share/ruphish/` | `$XDG_STATE_HOME/ruphish/`, otherwise `~/.local/state/ruphish/` |
| macOS | `~/Library/Application Support/ruphish/config.json` | `~/Library/Application Support/ruphish/` | `~/Library/Logs/ruphish/` |

Empty or relative Linux XDG directory variables use the listed home-directory fallback. Startup prints the selected configuration, SQLite and log paths. The generated file contains absolute paths; relative filesystem paths you later enter in this automatically discovered file resolve against its directory. Configuration creation uses private file permissions and publishes a complete file without replacing an existing one. Invalid configuration, denied access and database failures stop startup instead of resetting settings or choosing another database.

Use `ruphish --config /path/to/config.json` for an explicit installation, including Docker. That file must already exist; Ruphish does not create a fallback per-user installation. For compatibility, relative paths inside an explicitly selected configuration resolve against the process working directory. A local `./config.json` is only selected with `--config ./config.json`.

Generated defaults come from `config.example.json` embedded at build time; a loose example file is never read at runtime. These are not decoder defaults: omitted legacy strings start empty, booleans false, and lists empty. An incomplete configuration can therefore be rejected even though the generated file works. Operators can select MySQL or PostgreSQL by editing the configuration and restarting; moving data between providers is an operator task.

For an explicit installation, copy this minimal SQLite configuration into your chosen file and adjust the paths and listeners:

```json
{
  "admin_server": {
    "listen_url": "127.0.0.1:3333",
    "use_tls": true,
    "cert_path": "ruphish_admin.crt",
    "key_path": "ruphish_admin.key"
  },
  "phish_server": {
    "listen_url": "127.0.0.1:8081",
    "use_tls": false,
    "cert_path": "ruphish_public.crt",
    "key_path": "ruphish_public.key"
  },
  "db_name": "sqlite3",
  "db_path": "ruphish.db",
  "logging": { "filename": "ruphish.log", "level": "info", "max_size_bytes": 10485760, "max_files": 5 }
}
```

## Top-level fields

| Field | Type | Meaning |
| --- | --- | --- |
| `admin_server` | object | Administration listener and administrative request/outbound settings |
| `phish_server` | object | Public listener; only the four listener fields below are accepted |
| `db_name` | string | Explicit `sqlite3`, `mysql`, or `postgres`; no fallback from an invalid provider |
| `db_path` | string | SQLite file, or supported legacy MySQL TCP DSN; empty with typed server settings |
| `db_sslca_path` | string | Legacy MySQL CA path; incompatible with typed MySQL settings and other providers |
| `mysql` | object/null | [Typed MySQL settings](databases/mysql.md) |
| `postgres` | object/null | [Typed PostgreSQL settings](databases/postgresql.md) |
| `contact_address` | string | Contact address exposed by the public transparency response |
| `logging` | object | Output file and level |
| `domain_verification` | object | Installation-owned DNS/HTTPS/proxy policy |
| `test_flag` | boolean | Accepted for configuration compatibility; cannot enable a test mode |

All providers use embedded migrations. External migration directories and the Gophish `migrations_prefix` field are unsupported. Omit that field when translating a Gophish configuration; supplying it, even as an empty string, is rejected. See [migration from Gophish](../migration/from-gophish.md).

## Listener fields

| Field | Type/default | Meaning |
| --- | --- | --- |
| `listen_url` | string/empty | Required nonempty binding address for each selected listener, such as `127.0.0.1:3333` |
| `use_tls` | boolean/false | Enable TLS for this listener |
| `cert_path` | string/empty | Certificate path, required with TLS |
| `key_path` | string/empty | Private-key path, required with TLS |

Additional `admin_server` fields:

| Field | Type/default | Meaning |
| --- | --- | --- |
| `csrf_key` | string/empty | Configured CSRF key; changing it invalidates earlier CSRF tokens |
| `allowed_internal_hosts` | string array/empty | Exact IP/CIDR exceptions for [outbound integrations](outbound-connections.md) |
| `trusted_origins` | string array/empty | Additional browser origins; legacy `host[:port]` uses the listener scheme, or provide an explicit HTTP(S) origin |

These additional fields are not accepted under `phish_server`. Proxy header trust is a separate public-domain setting, not a meaning of `trusted_origins`.

## Domain verification

| Field | Default | Meaning |
| --- | --- | --- |
| `resolvers` | `[]` | System DNS, or explicit resolver socket addresses such as `127.0.0.1:5353` |
| `allowed_networks` | `[]` | Explicit CIDRs for private verification destinations; metadata/link-local and unspecified targets remain prohibited |
| `trusted_ca_path` | `null` | Optional PEM CA for an internal public listener; hostname validation remains enabled |
| `trusted_proxies` | `[]` | CIDRs of public-listener peers authorized to supply overwritten forwarded host/protocol headers |

Resource API clients cannot change these installation settings. See [listeners and TLS](listeners-and-tls.md) for proxy requirements.

## Logging and environment

Omitting `logging.filename` means stderr only; generated configurations select `ruphish.log` in the OS log directory. A configured file is appended to and rotated, with logs also written to stderr. Positive integer `logging.max_size_bytes` defaults to `10485760` (10 MiB); positive integer `logging.max_files` defaults to `5` archives in addition to the active file. Missing or null settings use these defaults unless an earlier duplicate legacy field already set the value. A single oversized record is kept intact. Oldest archives are removed to meet the configured count; changes take effect after restart. `logging.level` defaults to the info behavior; accepted names are empty, `trace`, `debug`, `info`, `warn`, `warning`, `error`, `fatal`, and `panic`, case-insensitively. Fatal/panic levels map to error severity. See [logging](logging.md).

`GOPHISH_INITIAL_ADMIN_PASSWORD` and `GOPHISH_INITIAL_ADMIN_API_TOKEN` are preserved bootstrap environment names. They are not configuration keys. Supply bootstrap credentials privately and remove them from the shell after setup. When no initial password is supplied, the application prints the generated temporary administrator password to startup stderr, including noninteractive Docker output. Supplied passwords and API tokens are never printed; generated passwords are not sent to the application log file. Typed database `password_env` fields name their own environment variable; there is no generic environment override for every JSON field.

## Parsing behavior

Unknown fields and invalid known-field types are rejected. Legacy configuration names are case-insensitive, and duplicate legacy object fields are merged in input order. Null legacy scalar values leave the current value intact; null lists clear those lists, and null logging resets the logging object. Prefer unique canonical lowercase keys instead of relying on merge behavior. Typed database/domain objects use their declared field names and reject unknown fields.
