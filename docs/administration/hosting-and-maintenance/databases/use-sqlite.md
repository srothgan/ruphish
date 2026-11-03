---
title: Use SQLite
description: Locate the default database, choose a persistent path, and preserve the file during maintenance.
sidebar:
  order: 26
---

SQLite is the default database. It runs inside Ruphish, so you do not need a database server, database password, or separate connection setup.

## Find the database

The default native installation stores `ruphish.db` in your platform's [data directory](../configuration-reference.md#default-locations-and-explicit-configuration). The Docker image stores it at `/var/lib/ruphish/ruphish.db`, which the Compose examples keep in the `ruphish-data` volume.

For an explicit Linux service configuration, use:

```json
{
  "db_name": "sqlite3",
  "db_path": "/var/lib/ruphish/ruphish.db"
}
```

These are database fields to merge into the complete configuration. Keep the listener and other settings. Remove MySQL or PostgreSQL settings and `db_sslca_path` when selecting SQLite.

The service account must be able to write the database and its containing directory. SQLite can create companion files beside the main database, so read/write permission on the `.db` file alone is insufficient.

## Move the database

1. Stop every Ruphish process using this database.
2. Back up the database and its directory using [Back up and restore](../back-up-and-restore.md).
3. Copy the stopped database to the new persistent location and give the service account access.
4. Update `db_path` and restart Ruphish.
5. Sign in and check that the expected resources and campaigns are present.

If Ruphish shows a fresh installation instead, stop it and check the selected path. A new empty path creates a new database; it does not find and move your previous data automatically.

## Startup and upgrades

Ruphish initializes an empty database and recognizes supported existing schemas before using them. Unknown histories, schema drift, or corruption stop startup. Do not delete migration records to bypass an error.

Supported SQLite upgrades create a verified pre-upgrade snapshot. That snapshot protects the upgrade point only; it is not a recurring backup of later work. Follow [Upgrade Ruphish](../upgrade-ruphish.md) and keep your own scheduled backups.

Stop Gophish before a [Gophish conversion](../../../migration/migrate-the-database.md). Ruphish's startup coordination does not stop an older application from writing to the file.
