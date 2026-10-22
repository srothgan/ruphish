---
title: Use SQLite
description: Configure a local database file and preserve it during upgrades and recovery.
sidebar:
  order: 2
---

SQLite is the sample configuration's local storage provider:

```json
{
  "db_name": "sqlite3",
  "db_path": "ruphish.db",
  "migrations_prefix": "db/db_"
}
```

These fields belong alongside your listener/logging settings. `db_path` must be nonempty. Remove typed server settings and `db_sslca_path`. Relative paths use the process working directory.

Startup recognizes the schema before using the file. Unknown, future, inconsistent, or corrupt populated databases fail instead of being initialized as empty. Supported upgrades preserve a verified pre-upgrade snapshot before applying changes. A current schema is not backed up again on every restart.

The startup lock coordinates cooperating Rust processes; it does not prove that a Gophish process has stopped. Stop old writers before upgrading. Separate admin/public processes can share an already prepared database.

New database and backup files receive private permissions. Check permissions on the containing directory and any operator-supplied copies as well. For routine copies and restore procedure, use [backup and recovery](../backup-and-recovery.md).
