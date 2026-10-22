---
title: Configuration reference
description: Configure listeners, databases, domain verification, logging, and startup secrets.
sidebar:
  order: 2
---

Ruphish reads JSON from the path selected by `--config`, defaulting to `./config.json`. Paths inside it resolve against the process working directory, not the configuration file's directory. Changes take effect at restart; there is no hot-reload contract.

Start from `config.example.json`. Its sample values are not decoder defaults: omitted legacy strings start empty, booleans false, and lists empty. An incomplete configuration can therefore be rejected even though the sample works.

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
| `migrations_prefix` | string | Legacy migration-directory compatibility setting; normally `db/db_` from the example |
| `contact_address` | string | Contact address exposed by the public transparency response |
| `logging` | object | Output file and level |
| `domain_verification` | object | Installation-owned DNS/HTTPS/proxy policy |
| `test_flag` | boolean | Accepted for configuration compatibility; cannot enable a test mode |

SQLite external migration directories must match the verified migration set; they are not arbitrary startup SQL. Server databases use embedded application migrations. See the provider guides before changing migration settings.

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

`logging.filename` defaults to empty (stderr only). A configured file is appended to, with logs also written to stderr. `logging.level` defaults to the info behavior; accepted names are empty, `trace`, `debug`, `info`, `warn`, `warning`, `error`, `fatal`, and `panic`, case-insensitively. Fatal/panic levels map to error severity. See [logging](logging.md).

`GOPHISH_INITIAL_ADMIN_PASSWORD` and `GOPHISH_INITIAL_ADMIN_API_TOKEN` are preserved bootstrap environment names. They are not configuration keys. Supply bootstrap credentials privately and remove them from the shell after setup. The application never prints generated credentials. Typed database `password_env` fields name their own environment variable; there is no generic environment override for every JSON field.

## Parsing behavior

Unknown fields and invalid known-field types are rejected. Legacy configuration names are case-insensitive, and duplicate legacy object fields are merged in input order. Null legacy scalar values leave the current value intact; null lists clear those lists, and null logging resets the logging object. Prefer unique canonical lowercase keys instead of relying on merge behavior. Typed database/domain objects use their declared field names and reject unknown fields.
