---
title: Verification and rollback
description: Verify migrated data and clients before cutover, and restore safely when checks fail.
sidebar:
  order: 900
prev: {"label": "Migration overview", "link": "/docs/migration/"}
next: false
---

A ready server proves startup completed; it does not prove that all data, client behavior, or external workflows meet your requirements. Compare a rehearsal with the source before converting the final database.

## Verify the shared database migration

| Check | Expected evidence |
| --- | --- |
| Selected database | Expected account/resource/campaign IDs and counts are present; the application did not silently open a new empty location. |
| Account access | Existing credentials/API keys work where the account is eligible, and locks and permissions behave as intended. |
| Shared visibility | Reviewers understand and accept which accounts can now access resources and campaigns created by others. |
| Resources and audiences | Names, recipient fields, group membership, attachments, and campaign recipient snapshots match the source where preservation is expected. |
| Dates and lifecycle | Scheduled times, event times, completion state, and any pre-UTC MySQL conversion agree with independently recorded source values. |
| Data transformations | Cleared participant values/location and missing historical client/attribution fields agree with [conversion rules](what-happens-to-your-data.md), rather than being mistaken for unexplained data loss. |
| Pending deliveries | Every runnable, blocked, unknown, or unresolved legacy queue item has been reviewed before workers are enabled. |
| Integrations | Imported mailbox/webhook settings, feature switches, secrets, and processing boundaries have been reviewed. |
| Restart | Restarting the same release reopens the converted database without repeating the Gophish conversion or losing confirmed state. |

Use controlled services and recipients for protocol checks. Allow only the destinations needed for that test; do not simply remove all rehearsal restrictions. Verify one received message, its personalization/attachments, the original public URL routing, an allowed participant interaction, and any reporting/webhook behavior your installation uses.

## Verify the chosen client path

For UI users, check the tasks in [Using the new interface](using-the-new-interface.md): inspect resources, prepare a campaign, and read its results. A hosting operator should complete this with a representative ordinary account as well as any administrator checks.

For API integrations, use the [endpoint inventory](api/index.md) to check every operation your client calls. Confirm request types, changed response parsers, HTTP methods/statuses, permission boundaries, conditional edits, result paging, and secret handling. Include a stale revision response and a failed operation; a successful list request is not an integration migration test.

## Cut over

1. Record the source backup and tested target release/image digest. Schedule downtime for the application and campaign links.
2. Stop Gophish and all other writers. Take a fresh consistent backup; the earlier rehearsal copy is no longer the production source.
3. Repeat the verified database procedure on the final working copy. Keep outbound work restricted until pending deliveries and imported integration settings have been checked.
4. Apply production listener, reverse-proxy, TLS, and domain configuration. Verify URLs already sent in campaign messages.
5. Point the adapted API clients and users to Ruphish. Do not let an old Gophish client keep making writes with an incompatible contract.
6. Enable the reviewed external work deliberately. Record when new Ruphish writes, sends, reports, and webhook deliveries begin.

Keep the original application/configuration and backup available until the migration is accepted. There is no general database downgrade command.

## If verification fails

Stop Ruphish and prevent further writes or external work. Preserve the failed working database, startup errors, and migration recovery artifacts. Do not delete schema history or an upgrade journal to make startup succeed.

For SQLite, restore the untouched backup to a separate path and configure the matching Gophish installation to use it. For MySQL, restore the pre-conversion dump/snapshot into a separate database and point Gophish there. Validate that restored installation in isolation before directing traffic back. See [Back up and restore](../administration/hosting-and-maintenance/back-up-and-restore.md).

Restoration discards Ruphish changes made after the backup. It also cannot unsend SMTP-accepted messages, retract delivered webhooks, undo deleted mailbox messages, or recover participant events written only to the discarded database. Reconcile those effects and pending work before resuming Gophish so rollback does not duplicate delivery or silently lose observations.

## Record the outcome

Keep the source/target versions, database/server versions, backup location, commands used, errors, and accepted differences. Distinguish checks actually performed from assumptions. In particular, a schema/type check does not establish that a real receiver accepted a message or that a browser page behaved correctly.
