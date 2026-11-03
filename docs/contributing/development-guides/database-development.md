---
title: Database development
description: Change provider operations, add migrations, and preserve atomicity and recovery.
sidebar:
  order: 230
---

SQLite, MySQL, and PostgreSQL implement application-owned persistence interfaces. Concrete pools and SQL stay in `ruphish-adapters`; application code describes the operation and its outcomes. For installation configuration, use [database administration](../../administration/hosting-and-maintenance/databases/index.md).

## Change a provider operation

1. Find the existing application interface and equivalent provider implementations. Change the shared contract first if the operation's meaning changes.
2. Keep authorization, revision comparison, parent/child replacement, attribution, and related event/outbox writes in one atomic operation. An HTTP handler must not assemble a transaction from several independent writes.
3. Return the confirmed record and any display identities through the existing `attribution::Saved<T>` boundary. A required identity lookup failure must roll back the write rather than fail response construction after commit.
4. Implement equivalent guarantees for every provider. SQL syntax and locking differ, but caller-visible outcomes must agree.
5. Extend shared provider scenarios and add provider-specific failure cases where the implementation differs. Test through the facade or real executable; use independent SQL for setup, observation, and fault injection.

Read an authorized bounded snapshot before rendering, export generation, or external network work. Release the database transaction before those expensive operations. Persist work ownership and outcome transitions through the provider's atomic contracts.

## Preserve shared-resource semantics

Historical `user_id` and `owner_id` columns on shared resources store creator attribution, not private access scope. Accounts, sessions, caller-scoped launch idempotency, and worker leases retain their distinct ownership rules. Deleting an account must not delete shared work.

Imported duplicate names are retained. New shared names are checked under the management write lock, and ambiguous name lookups are rejected. Do not introduce a global unique constraint or rename every ownership column without addressing these different meanings and imported data.

## Failure and uncertainty

A transaction can commit even when its acknowledgement is lost. Preserve typed uncertain outcomes and reconcile through durable operation identity. Do not turn every transport failure into a definite rollback or automatically resend a possibly accepted SMTP message.

Test the relevant cancellation, lock-timeout, pool-recovery, competing-writer, stale-generation, and process-restart cases. SQLite acquires its write transaction before reading the current revision. Server providers serialize short management writes and lock relevant rows before comparing and replacing state. Worker leases remain separate from configuration revisions.

## Add a migration

Released migrations are immutable. The initial schemas are `crates/ruphish-adapters/migrations/{sqlite,mysql,postgres}/1_baseline.sql`. Append forward migrations when changing a released schema; do not edit a released file or its recorded checksum.

The current providers construct their SQLx migration lists explicitly in `src/sqlite/migrations.rs`, `src/mysql/migrations.rs`, and `src/postgres/migrations.rs`. Adding a SQL file to a directory alone does not register it.

For a schema change:

1. Add the forward SQL for every affected provider and register it in that provider's migrator with an ordered version.
2. Trace fresh initialization, upgrade from each supported Ruphish schema, and startup with an already-current database.
3. Update the schema verification expectations and any code that assumes the baseline is the complete current schema. Include indexes, views, constraints, defaults, and provider metadata.
4. Preserve Gophish conversion into a valid Ruphish schema and its subsequent application migrations. Check recovery of interrupted MySQL publication and SQLite rollback/reopen.
5. Verify fresh and upgraded schemas agree, data is preserved or deliberately transformed, and modified/unknown migration histories are rejected.

SQL files are pinned to LF through Git attributes so checksums remain stable across platforms. Never initialize populated unrecognized storage as an empty installation.

## Update schema verification

| Provider | Verification owner |
| --- | --- |
| SQLite | `src/sqlite/schema.rs` compares schema objects with an independent in-memory database built from the registered migrations |
| MySQL | `src/mysql/schema_inventory.sql`, `src/mysql/schema.json`, and validation in `src/mysql/migrations.rs` |
| PostgreSQL | The `INVENTORY` query and verification in `src/postgres/migrations.rs`, `schema.json`, and checked view definitions |

All paths in this table are relative to `crates/ruphish-adapters/`. Generate changed server inventories from a fresh disposable schema after applying the intended migrations, using the same inventory query and normalization as the verifier. Exclude database names and data-dependent counters. For MySQL, use `SET NAMES utf8mb4 COLLATE utf8mb4_0900_bin`, matching application connections.

The inventories verify the schema independently of the migration journal. Do not hand-adjust expected metadata simply to silence a startup error. Check valid schemas on both supported server versions and deliberately alter schema objects to verify rejection.

## Maintain Gophish conversion

SQLite's `legacy.rs` recognizes historical schemas and converts their data; `startup.rs` owns locking, verified backups, and connection lifetime. MySQL's `preflight.rs` recognizes source schemas, `upgrade.rs` converts into staging tables, and `migrations.rs` publishes or resumes the conversion. PostgreSQL has no Gophish importer.

Keep original Gophish SQL in `tests/fixtures/sqlite-legacy/` and `tests/fixtures/mysql-legacy/`, with provenance and license notices. Runtime recognition inventories and independent fixtures must agree. Preserve supported identities, hashes, attribution, dates, queue dispositions, and service configuration while applying the documented privacy transformations.

Use the [migration guide](../../migration/index.md) for operator-visible behavior. Development tests must additionally challenge interrupted conversion, malformed or altered schemas, unknown histories, and reopen after success or failure.

## Verify the change

Use [database-provider tests](../testing/database-providers.md) for focused checks, server preparation, version matrices, and cleanup. Run affected consumers when changing shared semantics. Keep [legacy-test obligations](../../../tests/legacy-test-map.toml) accurate and report ignored server cases separately from executed results.
