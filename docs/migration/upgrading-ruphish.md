---
title: Upgrade Ruphish
description: Upgrade application and database state together while retaining a recoverable previous installation.
sidebar:
  order: 5
---

Upgrade the native executable, UI distribution, and compatible database state as one installation. The server verifies the browser asset manifest at startup; mixing an old distribution with changed inputs is not a valid upgrade check.

1. Record the current and target revisions, toolchains, database provider/version, configuration, and integration state. Review target changes for migrations, configuration changes, and API differences.
2. Rehearse on a consistent copy with outbound effects contained. Use the existing [local fixture](../contributing/testing/local-fixture.md) for synthetic end-to-end exercises, not as a substitute for testing your own schema history.
3. Stop writers and workers, back up the database and required configuration/keys, and verify restoration. Keep the previous executable and matching UI distribution available.
4. Build the target source with its pinned toolchain and lockfiles. Stop the running process before replacing the executable or promoting browser assets.
5. Start with the target configuration. Confirm database migration, asset verification, listeners, and worker startup in logs.
6. Complete [verification](verification-and-rollback.md) before resuming ordinary campaigns and integrations.

Do not edit migration history or schema metadata to bypass a startup rejection. A changed migration checksum, unexpected table definition, or dirty upgrade journal needs investigation. Application migrations are distinct from a database-server major-version upgrade; plan those separately.

No general database downgrade command is documented. Recovery uses the compatible previous application and a verified pre-upgrade backup. Account password changes and newly written state can prevent a newer database from being usable by an older application even when it opens successfully.

## Observation schema upgrade

SQLite schema 11 and server schema 5 give events canonical kinds, explicit recipient attribution, source metadata, and linked submission receipts. Existing IDs and timestamps remain. Historical receipts without an event link remain valid retry tombstones; ambiguous historical recipient attribution remains unresolved. Capture flags are removed from active page/campaign-policy storage and ignored in API inputs. Campaign summaries remain unique by recipient; recipient analysis counts repeats. Retain the pre-upgrade backup: older executables cannot read the new schema, and MySQL DDL interruptions require recovery from that backup.

Before upgrading MySQL, grant the service account database-scoped `CREATE VIEW` and `SHOW VIEW` in addition to its existing migration privileges. The migration creates and validates the `event_history` compatibility view.
