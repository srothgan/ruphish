---
title: Back up and recover an installation
description: Preserve database and configuration state and verify a restore before cutover.
sidebar:
  order: 9
---

A useful backup includes the database state and the configuration/secrets needed to run it. Keep certificate keys, service-credential sources, and configuration under appropriate access controls. Administration assets can be rebuilt from matching source; do not substitute unrelated assets during recovery.

## SQLite

For a straightforward offline copy, stop every process using the database and use a database-consistent backup procedure. Do not copy only a live `.db` file while ignoring its WAL. Keep a recoverable original and restore into a separate path for verification.

Recognized upgrades create a verified `ruphish-pre-upgrade-*.sqlite3` snapshot including committed WAL content. Hidden `.ruphish-backup-tmp-*` files left by interruption are not advertised as verified recovery copies. The snapshot is a pre-upgrade point, not a rolling backup of later work.

## Server databases

Use your database server's consistent backup/restore process and verify the backup before an application upgrade. MySQL legacy-upgrade recovery tables are additional migration evidence, not a replacement for an operator-managed backup. PostgreSQL transactional DDL is not a backup facility.

## Verify a restore

Restore into a separate path/database and keep outbound delivery controlled. Check account access, ownership, resource content, campaign history, schedules, and integration configuration before selecting it for normal operation. Do not let a recovery check accidentally resend queued campaigns.

Recovery copies can retain legacy captured values and service credentials. Protect or retire them according to your retention requirements. Upgrading active records does not sanitize old copies.

For Go rollback and version-specific constraints, follow [migration verification and rollback](../migration/verification-and-rollback.md).
