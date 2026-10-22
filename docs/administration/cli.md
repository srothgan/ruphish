---
title: Command-line reference
description: Select configuration, listeners, delivery workers, and help output.
sidebar:
  order: 3
---

Run the executable from the working directory that owns your relative configuration, asset, and database paths. Replace the dummy configuration path below. In a source checkout:

```sh
export RUPHISH_CONFIG='/path/to/config.json'

cargo run -p ruphish --locked -- --config "$RUPHISH_CONFIG" --mode all
```

The `--` separates Cargo options from Ruphish options. A directly invoked development binary accepts the same application arguments. `RUPHISH_CONFIG` is an example-shell variable passed through `--config`; setting it alone does not change which configuration Ruphish loads.

| Option | Default | Behavior |
| --- | --- | --- |
| `--config PATH` | `./config.json` | Load configuration from PATH |
| `--mode all` | `all` | Run administration and public listeners |
| `--mode admin` | — | Run administration only |
| `--mode phish` | — | Run public handling only |
| `--disable-mailer` | false | Disable campaign queue execution in this process; IMAP is independent |
| `--no-disable-mailer` | — | Compatibility alias enabling the mailer; conflicts with `--disable-mailer` |
| `-h`, `--help` | — | Print ordinary help to stderr and exit |
| `--help-long` | — | Print extended help to stdout and exit |
| `--help-man` | — | Print manual-page text to stdout and exit |
| `--version` | — | Print the application version to stderr and exit |

Only modes containing the administration listener run the campaign worker. Inactive listeners do not require usable addresses, certificates, or administration assets. The mailer switches are flags, not `--disable-mailer=false` assignments.

Successful startup emits `RUPHISH_READY` with bound-address JSON to stderr, independently of logging level. Port zero is available for isolated tests. Normal Ctrl+C, Windows Ctrl+Break, and Unix SIGTERM shutdown succeed; fatal initialization/runtime failure exits with status 1. Invalid CLI syntax is a parser failure, not successful readiness.

See [process operation](running-the-service.md) for working-directory and shutdown considerations.
