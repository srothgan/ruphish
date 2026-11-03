---
title: Databases
description: Choose SQLite, MySQL, or PostgreSQL and understand what changing storage involves.
sidebar:
  order: 24
---

Ruphish uses one database for accounts, company settings, resources, campaigns, delivery state, and results. SQLite is included and needs no separate database server. Choose MySQL or PostgreSQL when you want to use your organization's server-database operations and backup tooling.

| Provider | Choose it when | Supported setup |
| --- | --- | --- |
| [SQLite](sqlite.md) | You want a database file managed with the Ruphish host or Docker volume. | Default for new installations; recognized Gophish SQLite databases can be converted. |
| [MySQL](mysql.md) | Your team already operates MySQL. | MySQL 8.4 and 9.7 LTS lines; new installations and recognized Gophish MySQL conversion. MariaDB is not supported. |
| [PostgreSQL](postgresql.md) | Your team already operates PostgreSQL. | PostgreSQL 17 and 18; new Ruphish installations. |

For a server database, create the database and service account before starting Ruphish. Ruphish creates and updates its application tables automatically, but does not install the database server or create its login account.

## Keep the data when changing configuration

Changing `db_name`, a database name, or a SQLite file path selects different storage. It does not copy accounts or campaigns to that storage. Ruphish has no cross-engine data-transfer tool.

If you need to move a database between hosts, use a backup and restore for the same provider, then update its connection settings. See [Back up and restore](../backup-and-restore.md).

Fresh setup, a Ruphish release upgrade, and a Gophish conversion are different tasks. Use [Upgrade Ruphish](../upgrades.md) for an existing Ruphish installation and [Migration from Gophish](../../../migration/from-gophish.md) for a Gophish database. Ruphish rejects unrecognized or inconsistent database histories instead of treating them as empty installations.
