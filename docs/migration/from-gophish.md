---
title: Migrate the database
description: Prepare a recoverable Gophish conversion and choose the SQLite or MySQL procedure.
sidebar:
  order: 30
---

Ruphish recognizes supported Gophish databases and converts them at startup. There is no separate migration command, SQL directory to install, or API request that imports the whole database. The selected database changes in place, so begin with a copy.

## Prepare

1. Record the Gophish version, database engine and server version, database location, configuration, public URLs, and enabled integrations. Record resource and campaign counts and identify queued or active campaigns.
2. Read [Changes to review](differences-from-gophish.md), particularly shared access and the treatment of captured participant data. API maintainers should also inventory their calls using [API migration](api/index.md).
3. Stop every Gophish process using the database before taking the final backup. During a rehearsal, you may resume Gophish only after obtaining a consistent copy; discard the rehearsal database before the final migration.
4. Back up the database, Gophish executable/version, configuration, certificates/private keys, and external assets. Keep the backup separately from the working copy and confirm it can be restored.
5. Prepare an isolated environment. Block its access to production SMTP servers, webhook receivers, reporting mailboxes, and campaign-domain verification targets. A restored installation contains live credentials and enabled integrations.
6. Install the target Ruphish release, then follow the database-specific procedure below.

`--disable-mailer` disables campaign queue execution only. It does not disable reporting mailboxes, webhooks, domain checks, or interactive network actions. An isolated network or host-level outbound restrictions are required for a rehearsal using copied production settings.

## Choose the procedure

| Database | Procedure | Recovery mechanism |
| --- | --- | --- |
| SQLite | [Migrate SQLite](database/sqlite.md) | Startup creates a verified pre-conversion database snapshot beside the working file, then converts within a transaction. |
| MySQL | [Migrate MySQL](database/mysql.md) | Startup copies into staging tables and publishes them while retaining original tables and an upgrade journal. Keep an independent database backup. |

Neither procedure transfers data between database engines. A MySQL server upgrade, when required, is a separate prerequisite; do not combine it with the first Ruphish conversion attempt.

## After conversion

Complete [Verification and rollback](verification-and-rollback.md) before allowing production traffic or workers. Review [What happens to your data](data-changes.md) for changes that an unchanged row count cannot reveal.

Continue with [Using the new interface](using-the-interface.md) or [API migration](api/index.md). For later Ruphish release updates, use [Upgrade Ruphish](../administration/hosting-and-maintenance/upgrades.md).
