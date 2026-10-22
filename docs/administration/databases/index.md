---
title: Choose a database
description: Select a storage provider and understand provisioning and migration boundaries.
sidebar:
  order: 1
---

Ruphish selects one provider at startup. It does not fall back to another engine after a configuration or connection failure.

| Provider | Configuration | Current implementation boundary |
| --- | --- | --- |
| [SQLite](sqlite.md) | `db_name: "sqlite3"` and a file path | Default local example; supported legacy file upgrades |
| [MySQL](mysql.md) | `db_name: "mysql"` and typed settings or supported legacy TCP DSN | MySQL 9.7 LTS primary, 8.4 LTS compatibility floor; recognized legacy upgrades |
| [PostgreSQL](postgresql.md) | `db_name: "postgres"` and typed settings | PostgreSQL 18 primary, 17 compatibility floor; fresh Ruphish baseline, no Gophish PostgreSQL history |

These version boundaries come from the provider implementation and fixture configuration; a new environment still needs qualification. MariaDB and unverified server major versions are not accepted as substitutes.

Create server databases and service accounts before starting Ruphish. The application performs its schema preparation, but does not provision database servers or accounts. There is no cross-engine transfer or live provider-switching tool. Read [migration](../../migration/index.md) before pointing a new executable at populated storage.
