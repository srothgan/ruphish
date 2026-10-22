---
title: Verify a migration and plan rollback
description: Check preserved data, controlled workflows, and the boundary between database recovery and external effects.
sidebar:
  order: 6
---

A successful startup is only the first check. Compare the restored installation against a recorded baseline before allowing normal work.

| Check | Evidence to retain |
| --- | --- |
| Accounts and ownership | Existing account can sign in; roles, locks, IDs, and owner isolation remain correct |
| Resources | Group membership, content, attachments, sender settings, and modification timestamps match expected state |
| Campaign history | Campaign/recipient relationships, lifecycle state, event times, and reporting flags are accounted for |
| Queued work | Scheduled, retryable, blocked, and uncertain deliveries have an explicit disposition |
| Template rendering | A controlled received message has the expected text, HTML, personalization, attachment, and origin |
| Public interactions | Verified origin serves the intended page and records allowed events |
| Reports and integrations | Analysis/export interpretation is correct; webhook signatures and mailbox matching work against controlled services |
| Restart | A deliberate restart preserves configuration, sessions as intended, and durable work without an unintended resend |
| Recovery | The pre-upgrade backup opens with the compatible previous installation in isolation |

Record commands, versions, exit statuses, and observations. Separate source inspection, executed protocol tests, and manual browser review. The contributor [manual UI checks](../contributing/testing/manual-ui.md) include browser-specific privacy checks that a direct HTTP test cannot establish.

## If verification fails

Stop the target's workers and writers. Preserve logs and the failed database for diagnosis. Do not repeatedly start alternate application versions against it or remove upgrade journals to make startup proceed.

Restore the verified backup to a separate location, configure the compatible previous application to use it, and validate it before directing traffic there. For SQLite, include committed WAL state through a consistent backup; copying only the live main file can lose committed changes. For MySQL, retain original tables and the upgrade journal until the documented recovery path is complete. See [backup and recovery](../administration/backup-and-recovery.md).

Reconcile external effects since the backup: SMTP-accepted messages, delivered webhooks, acknowledged/deleted mailbox messages, and participant events are not undone by database restoration. Prevent duplicate delivery while deciding what work can resume. A rollback plan is incomplete if it only describes copying files.
