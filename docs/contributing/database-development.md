---
title: Database development
description: Preserve shared persistence contracts while implementing provider-specific atomicity and recovery.
sidebar:
  order: 7
---

The adapter provider factory supplies application-owned persistence capabilities for SQLite, MySQL, and PostgreSQL. Concrete pools and SQL remain private to the adapter layer. Operational setup belongs in [database administration](../administration/databases/index.md); this page explains how to change persistence safely.

## Authoritative boundaries

Application/domain code owns workflow meaning, invariants, validation, and allowed transitions. Providers own authorized reads, atomic writes, transaction/lock boundaries, and durable claims or checkpoints. HTTP handlers should not reconstruct a transaction by calling several independent writes.

Shared rendering, SMTP/IMAP/webhook transport, and export generation run outside concrete provider modules. Read an authorized bounded snapshot, release the database transaction, then perform expensive generation or external work. Persist ownership and result transitions through the appropriate atomic contract.

## Failure and uncertainty

A transaction can commit even when the client loses the commit acknowledgement. Preserve typed uncertain outcomes and reconcile by durable operation identity. Do not convert every transport failure into a definite rollback, or automatically repeat a possibly accepted SMTP send.

Test cancellation, lock timeout, pool recovery, concurrent claims, stale generations, and process restart at the boundary where they matter. Account deletion must stay consistent with domain claims and other owned records. Related child replacement and outbox/event publication must remain atomic.

## Migrations and schema checks

Application migrations start at `crates/ruphish-adapters/migrations/{sqlite,mysql,postgres}/1_baseline.sql`. Each baseline defines the final release schema directly. Once released, it is immutable; append forward migrations rather than editing its checksum. All providers use SQLx application history. Git pins release SQL to LF on every platform so checksums are stable. Startup rejects unrecognized histories, modified checksums, and inconsistent schemas.

SQLite's `migrations.rs` owns application history, `schema.rs` verifies the complete schema, and `legacy.rs` recognizes Gophish shapes and converts source data. `startup.rs` owns locking, verified backups and connection lifecycle. MySQL's `preflight.rs` recognizes legacy shapes; `upgrade.rs` converts into final staging tables and `migrations.rs` owns publication/recovery. PostgreSQL initializes fresh schemas and has no Gophish importer.

Original Gophish SQL belongs only in `tests/fixtures/sqlite-legacy/` and `tests/fixtures/mysql-legacy/`, with provenance and license notices. Runtime source inventories recognize supported versions without replaying old SQL. Keep those inventories aligned with independent legacy fixtures. Preserve source-supported IDs, hashes, ownership, dates, queue dispositions and service configuration while applying the documented privacy transformations. Never initialize populated unrecognized storage as empty.

Schema inventory files are generated expectations tied to the release migrations. Regenerate them from a fresh disposable schema only after an intentional schema change, using the provider's checked-in inventory SQL. Exclude database names and data-dependent counters. Validate supported provider versions and test rejection of altered schemas, not just successful fresh initialization.

For MySQL, the inventory connection uses `SET NAMES utf8mb4 COLLATE utf8mb4_0900_bin`, matching application connections. Consult `crates/ruphish-adapters/src/mysql/schema_inventory.sql` and its `schema.json`; use the corresponding PostgreSQL sources for that provider. Do not hand-adjust metadata to hide a startup mismatch.

## Verification

Use the shared provider scenarios through the public facade and executable, with independent SQL only for setup, observation, and fault injection. Keep fixtures synthetic and isolated. Run the affected provider/version cases in [database-provider testing](testing/database-providers.md), plus shared consumers when changing shared semantics.

Preserve the legacy-test map's source identities and implementation status. An ignored server test is not passing provider evidence. Record exact commands and distinguish a setup failure from a failed executed assertion.

Conversion tests must compare the resulting application schema with fresh initialization, verify transformed data independently, and exercise reopen and interrupted publication. Use independent Gophish source fixtures and current application schemas. Cover physical query work and rejection of missing or altered indexes.
