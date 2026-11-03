---
title: Configuration reference
description: Find the configuration file and set process options through JSON or environment variables.
sidebar:
  order: 29
---

Ruphish reads a JSON configuration file at startup. Use it for listeners, database connections, logging, and network policy. [Company settings](../team-administration/company-settings.md), such as enabling webhooks, are saved separately in the database through the browser.

Restart Ruphish after changing process settings. With Docker Compose, run `docker compose up -d --wait` to apply changed environment values by recreating the container.

## Default locations and explicit configuration

Without `--config`, a native binary reads the following `config.json` and creates it with SQLite defaults if it does not exist:

| Platform | Configuration | Data and generated certificates | Logs |
| --- | --- | --- | --- |
| Windows | `%LOCALAPPDATA%\ruphish\config.json` | `%LOCALAPPDATA%\ruphish\data\` | `%LOCALAPPDATA%\ruphish\logs\` |
| Linux | `~/.config/ruphish/config.json` | `~/.local/share/ruphish/` | `~/.local/state/ruphish/` |
| macOS | `~/Library/Application Support/ruphish/config.json` | `~/Library/Application Support/ruphish/` | `~/Library/Logs/ruphish/` |
| Docker image | `/etc/ruphish/config.json`, supplied by the image | `/var/lib/ruphish/` | Container output by default |

On Linux, absolute `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_STATE_HOME` values replace the corresponding default base directories. Empty or relative values use the home-directory defaults. Startup prints the selected paths.

To use an existing file explicitly:

```sh
ruphish --config /etc/ruphish/config.json
```

That file must exist. A file named `config.json` in your current folder is not selected automatically; use `--config ./config.json` when you intend to use it.

| How the file is selected | Relative filesystem paths inside it resolve from |
| --- | --- |
| Default discovery, without `--config` | The directory containing the configuration file |
| Explicit `--config` | The process working directory |

Use absolute paths for services to make this distinction unambiguous. Ruphish does not overwrite an existing configuration or fall back to an empty database when loading fails.

The generated native configuration includes local HTTPS on port 3333, HTTP on port 8081, SQLite, and file logging. These generated values are not all implied when fields are omitted from a hand-written file. Start from a complete example in [Linux binary service](run-a-linux-binary-service.md) or use the [Docker image defaults](run-with-docker-compose.md), then edit the fields you need.

## Configuration precedence

1. The command-line `--config` option selects the file, or Ruphish uses default discovery.
2. Environment overrides replace values in that file.
3. Ruphish resolves paths and validates the result before starting.

Environment overrides do not rewrite the file. A malformed JSON file fails to load even if an environment override would replace one of its values. Unknown fields and invalid types are rejected.

## Top-level fields

| Field | Type | Meaning |
| --- | --- | --- |
| `admin_server` | Object | Browser interface, management API, and integration-network settings. |
| `phish_server` | Object | Campaign listener settings. |
| `db_name` | String | Required provider: `sqlite3`, `mysql`, or `postgres`. |
| `db_path` | String | SQLite file path; use an empty string with typed server-database settings. |
| `mysql` | Object or null | [MySQL connection settings](databases/use-mysql.md). |
| `postgres` | Object or null | [PostgreSQL connection settings](databases/use-postgresql.md). |
| `contact_address` | String | Contact address exposed by the public transparency response; empty when omitted. |
| `logging` | Object | Destination, severity level, and rotation settings. |
| `domain_verification` | Object | DNS, certificate trust, and proxy policy described below. |

Configure one database provider. Ruphish initializes or upgrades its supported schema at startup using migrations included in the executable. It does not accept an external migrations directory or a `migrations_prefix` field. Changing providers does not transfer existing data.

## Listener fields

Both `admin_server` and `phish_server` accept these fields:

| Field | Type / value when omitted | Meaning |
| --- | --- | --- |
| `listen_url` | String / empty | Binding address, such as `127.0.0.1:3333`. Required for each listener selected by `--mode`. Do not include `http://` or a path. |
| `use_tls` | Boolean / `false` | Serve HTTPS on this listener. |
| `cert_path` | String / empty | PEM certificate file; required when TLS is enabled. |
| `key_path` | String / empty | PEM private-key file; required when TLS is enabled. |

Only `admin_server` accepts these additional fields:

| Field | Type / value when omitted | Meaning |
| --- | --- | --- |
| `csrf_key` | String / empty | Optional configured key for browser request protection. Changing it invalidates previous CSRF tokens. |
| `trusted_origins` | String array / `[]` | Additional browser origins, such as `https://ruphish.example.com`, allowed for protected requests. |
| `allowed_internal_hosts` | String array / `[]` | Exact IP addresses or CIDRs allowed for [internal integration connections](connect-to-internal-services.md). Hostnames are not allowlist entries. |

See [Network access and HTTPS](network-access-and-https.md) for complete proxy examples. `trusted_origins` does not configure public proxy trust.

## Domain-verification network policy

These fields belong inside `domain_verification`. They control how the host performs checks; the decision to require verification belongs to [company settings](../team-administration/company-settings.md).

| Field | Type / default | Meaning |
| --- | --- | --- |
| `resolvers` | String array / `[]` | Use system DNS, or supply resolver socket addresses such as `10.20.0.53:53`. |
| `allowed_networks` | String array / `[]` | Allow specified private destination CIDRs for verification. Metadata/link-local and unspecified addresses remain prohibited. |
| `trusted_ca_path` | String or null / null | Optional PEM certificate authority for checking an internal campaign website. Hostname validation remains enabled. |
| `trusted_proxies` | String array / `[]` | Source CIDRs of proxies allowed to supply the campaign's original host and protocol. |

## Logging fields

| Field | Type / default | Meaning |
| --- | --- | --- |
| `filename` | String / empty | Empty sends logs to stderr only. A file path enables file output as well as stderr. |
| `level` | String / `info` behavior | `trace`, `debug`, `info`, `warn`, or `error`. Names are case-insensitive. Empty uses `info`; `warning` means `warn`, and `fatal` or `panic` mean `error`. |
| `max_size_bytes` | Positive integer / `10485760` | Rotate a file at approximately 10 MiB by default; an individual record is kept intact. |
| `max_files` | Positive integer / `5` | Number of archives retained in addition to the active file. |

The generated native configuration chooses a file in the platform's log directory; the Docker image chooses stderr only. See [Health and logs](health-and-logs.md) for commands, retention, and failure handling.

## Environment overrides

Use uppercase names with underscores and no product prefix. Quote values in Compose, including booleans, numbers, and JSON arrays. Strings are literal; booleans must be `true` or `false`, and arrays use JSON syntax. Empty strings clear string values; `[]` clears a list.

| Name or prefix | Supported fields |
| --- | --- |
| `DB_NAME`, `DB_PATH`, `DB_SSLCA_PATH`, `CONTACT_ADDRESS` | Corresponding top-level fields |
| `ADMIN_SERVER_` | `LISTEN_URL`, `USE_TLS`, `CERT_PATH`, `KEY_PATH`, `CSRF_KEY`, `ALLOWED_INTERNAL_HOSTS`, `TRUSTED_ORIGINS` |
| `PHISH_SERVER_` | `LISTEN_URL`, `USE_TLS`, `CERT_PATH`, `KEY_PATH` |
| `LOGGING_` | `FILENAME`, `LEVEL`, `MAX_SIZE_BYTES`, `MAX_FILES` |
| `DOMAIN_VERIFICATION_` | `RESOLVERS`, `ALLOWED_NETWORKS`, `TRUSTED_CA_PATH`, `TRUSTED_PROXIES` |
| `MYSQL_` | `HOST`, `PORT`, `DATABASE`, `USERNAME`, `PASSWORD`, `PASSWORD_ENV`, `TLS`, `CA_PATH`, `MAX_CONNECTIONS`, `CONNECT_TIMEOUT_SECONDS`, `ACQUIRE_TIMEOUT_SECONDS`, `LOCK_TIMEOUT_SECONDS`, `LEGACY_TIMEZONE` |
| `POSTGRES_` | `HOST`, `PORT`, `DATABASE`, `USERNAME`, `PASSWORD`, `PASSWORD_ENV`, `TLS`, `CA_PATH`, `MAX_CONNECTIONS`, `CONNECT_TIMEOUT_SECONDS`, `ACQUIRE_TIMEOUT_SECONDS`, `LOCK_TIMEOUT_SECONDS`, `STATEMENT_TIMEOUT_SECONDS` |

For example, add this under the Compose `ruphish` service:

```yaml
environment:
  ADMIN_SERVER_TRUSTED_ORIGINS: '["https://ruphish.example.com"]'
  LOGGING_LEVEL: "info"
```

On a native Linux or macOS shell, an override can apply to one launch:

```sh
LOGGING_LEVEL=debug ruphish
```

In PowerShell:

```powershell
$env:LOGGING_LEVEL = 'debug'
ruphish
Remove-Item Env:LOGGING_LEVEL
```

The last PowerShell command runs after Ruphish exits. For services, put overrides in the service manager's environment configuration rather than an unrelated interactive terminal.

Unknown names within the listed configuration prefixes fail startup; unrelated environment variables are ignored. Invalid types and null boolean, number, or list overrides also fail. Errors do not echo supplied values.

Changing `DB_NAME` clears the previous provider's connection settings from the file. Supply the required settings for the new provider; switching to SQLite requires `DB_PATH`. Selecting the same provider preserves file values except where overridden. This selects storage but does not migrate data.

For database credentials, use either `PASSWORD` or `PASSWORD_ENV`, not both. A direct environment password replaces the file's `password_env`; an environment `PASSWORD_ENV` replaces its direct password. The variable named by `PASSWORD_ENV` should have a separate name outside these reserved prefixes, such as `DATABASE_SECRET`.

## Initial administrator credentials

`GOPHISH_INITIAL_ADMIN_PASSWORD` and `GOPHISH_INITIAL_ADMIN_API_TOKEN` are the supported bootstrap environment names. They retain the historical prefix and are not JSON fields. They initialize the first administrator; they are not a password-reset mechanism for an existing account.

When no initial password is supplied, Ruphish prints a generated temporary password to startup stderr, including container output. Supplied passwords and API tokens are not printed. Generated passwords do not go to the application log file, but can appear in Docker logs or the system journal. Remove bootstrap secrets from the service environment after initial setup.

## Compatibility fields

Existing Gophish configuration may contain a supported MySQL TCP connection string in `db_path` and its CA file in `db_sslca_path`. Do not combine these with typed MySQL settings or other providers. Prefer the [typed MySQL example](databases/use-mysql.md) for a new configuration.

`test_flag` is accepted for compatibility but does not enable a test mode. Legacy listener and top-level field names are case-insensitive, and repeated legacy objects can merge in input order. Use unique, lowercase keys and explicit values in maintained configurations. Typed database and domain-verification objects reject unknown fields. Follow [Migration from Gophish](../../migration/migrate-the-database.md) when translating an older configuration.
