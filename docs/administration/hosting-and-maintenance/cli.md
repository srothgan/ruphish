---
title: Command-line reference
description: Select configuration and listeners, control the campaign worker, and inspect command help.
sidebar:
  order: 30
---

Run the installed `ruphish` binary without arguments to use or create your [default configuration](configuration.md#default-locations-and-explicit-configuration):

```sh
ruphish
```

For a service using an existing configuration file:

```sh
ruphish --config /etc/ruphish/config.json --mode all
```

Replace the path with your own. Relative paths inside an explicitly selected configuration resolve from the process working directory.

## Options

| Option | Default | Effect |
| --- | --- | --- |
| `--config PATH` | Default configuration location | Read an existing file. An explicitly selected missing file is an error. |
| `--mode all` | `all` | Run the browser/API and campaign listeners. |
| `--mode admin` | — | Run the browser/API listener without the campaign listener. |
| `--mode phish` | — | Run the campaign listener without the browser/API listener. |
| `--disable-mailer` | Off | Disable campaign queue execution in this process. Reporting-mailbox polling is independent. |
| `-h`, `--help` | — | Print help to stderr and exit. |
| `--help-long` | — | Print extended help to stdout and exit. |
| `--help-man` | — | Print manual-page text to stdout and exit. |
| `--version` | — | Print the application version to stderr and exit. |

The compatibility flag `--no-disable-mailer` enables the mailer and cannot be combined with `--disable-mailer`. These are flags, not assignments such as `--disable-mailer=false`.

Only modes that include the administration listener run the campaign worker. Inactive listeners do not need usable addresses or TLS files. Use the default `all` mode for a single-process installation.

## Pass options through Docker Compose

The image already has the executable as its entrypoint. Add only its arguments under `command`, for example:

```yaml
command: ["--mode", "all", "--disable-mailer"]
```

Then apply the change with `docker compose up -d --wait`. Remove `--disable-mailer` and recreate the container to resume queue execution.

Disabling the mailer is not a complete isolation mode. It does not disable reporting mailboxes, webhook delivery, domain checks, or interactive connection tests. Keep restored data isolated from real external services during [recovery checks](backup-and-restore.md#verify-a-restore).

## Startup and shutdown

Successful startup emits `RUPHISH_READY` with the bound addresses to stderr, independently of log level. Use [readiness probes](monitoring.md#check-readiness) for ongoing monitoring.

Press **Ctrl+C** to stop a foreground process. Ruphish also handles Windows **Ctrl+Break** and Unix `SIGTERM` for graceful shutdown. A fatal initialization or runtime failure exits with status `1`; invalid arguments produce a command-line parser error. Allow the service time to stop before forcing termination.
