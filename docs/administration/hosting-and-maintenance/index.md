---
title: Hosting and maintenance
description: Run Ruphish reliably, make it reachable, and protect its data.
sidebar:
  order: 20
---

Use this section when you manage the computer, containers, network, or database that runs Ruphish. The commands require access to that environment. Managing an Administrator account in the browser does not grant server access.

## Set up a host

1. Run Ruphish with [Docker Compose](docker.md) or as a [Linux binary service](binary-service.md). Both use the released application with its browser interface included.
2. Configure [network access and HTTPS](network-and-https.md) so colleagues and campaign recipients can reach the right addresses.
3. Keep the default SQLite database or [choose a server database](databases/index.md).
4. Set up [backups](backup-and-restore.md) and [health and log monitoring](monitoring.md).

For a first trial on your own computer, use [Install Ruphish](../../getting-started/installation.mdx). The hosting guides build on that installation rather than requiring source code or development tools.

## Maintain the installation

| Task | Guide |
| --- | --- |
| Change ports, file locations, environment values, or other process settings | [Configuration reference](configuration.md) |
| Allow connections to an internal mail server or integration | [Internal services](outbound-connections.md) |
| Select listeners or disable campaign sending in a process | [Command-line reference](cli.md) |
| Install a newer release | [Upgrade Ruphish](upgrades.md) |
| Recover data or test a backup | [Back up and restore](backup-and-restore.md) |
| Diagnose a startup, connection, or delivery problem | [Troubleshooting](troubleshooting.md) |

Application accounts and feature switches belong to [Team administration](../team-administration/index.md). Moving an existing Gophish database to Ruphish has its own [migration guide](../../migration/from-gophish.md).
