---
title: Hosting and maintenance
description: Run Ruphish reliably, make it reachable, and protect its data.
sidebar:
  order: 20
---

Use this section when you manage the computer, containers, network, or database that runs Ruphish. The commands require access to that environment. Managing an Administrator account in the browser does not grant server access.

## Set up a host

1. Run Ruphish with [Docker Compose](run-with-docker-compose.md) or as a [Linux binary service](run-a-linux-binary-service.md). Both use the released application with its browser interface included.
2. Configure [network access and HTTPS](network-access-and-https.md) so colleagues and campaign recipients can reach the right addresses.
3. Keep the default SQLite database or [choose a server database](databases/index.md).
4. Set up [backups](back-up-and-restore.md) and [health and log monitoring](health-and-logs.md).

For a first trial on your own computer, use [Install Ruphish](../../getting-started/install-ruphish.mdx). The hosting guides build on that installation rather than requiring source code or development tools.

## Maintain the installation

| Task | Guide |
| --- | --- |
| Select a binary release, customize installation, or remove program files | [Native installer reference](native-installer-reference.md) |
| Change ports, file locations, environment values, or other process settings | [Configuration reference](configuration-reference.md) |
| Allow connections to an internal mail server or integration | [Internal services](connect-to-internal-services.md) |
| Select listeners or disable campaign sending in a process | [Command-line reference](command-line-reference.md) |
| Install a newer release | [Upgrade Ruphish](upgrade-ruphish.md) |
| Recover data or test a backup | [Back up and restore](back-up-and-restore.md) |
| Diagnose a startup, connection, or delivery problem | [Troubleshooting](troubleshoot-the-installation.md) |

Application accounts and feature switches belong to [Team administration](../team-administration/index.md). Moving an existing Gophish database to Ruphish has its own [migration guide](../../migration/migrate-the-database.md).
