---
title: Migrate from Gophish
description: Rehearse a supported same-engine database upgrade and verify workflows before cutover.
sidebar:
  order: 4
---

Use this procedure for a recognized legacy SQLite or MySQL database. First read the [compatibility boundary](compatibility.md) and the provider's [SQLite](../administration/databases/sqlite.md) or [MySQL](../administration/databases/mysql.md) requirements. There is no documented automatic cross-engine migration.

## Prepare a recoverable rehearsal

1. Record the Gophish build, database engine/version, migration history, configuration, public origins, and external integrations. Inventory scheduled and active campaigns and any messages whose delivery is uncertain.
2. Stop all writers and workers for the backup. Take a consistent database backup using the engine's supported method, and save configuration, certificates/keys, and needed assets separately. Confirm that the backup can be restored.
3. Restore into an isolated rehearsal location. Restrict outbound access so the copy cannot deliver campaign mail, call production webhooks, or alter reporting mailboxes. `--disable-mailer` controls campaign sending; it is not a universal switch for all integrations.
4. Build the target Ruphish checkout and prepare its current UI distribution. Translate configuration using the [configuration reference](../administration/configuration.md), including TLS trust, database credentials, and origin verification settings.
5. Start Ruphish against the copy and inspect startup logs. An unrecognized or inconsistent history is a stop condition, not a reason to clear migration tables or create an empty replacement database.

SQLite creates a verified snapshot before upgrading supported legacy state. MySQL uses staged tables and retained originals with an upgrade journal. These protections supplement your own backup; preserve the original until verification is complete.

## Verify data and workflows

### What carries across

The same-engine upgrade transforms a recognized database in place. It is not a JSON import that recreates every object with new IDs. The following describes source-supported upgrade behavior; verify it against your database copy before cutover.

| Data | Upgrade treatment | Follow-up |
| --- | --- | --- |
| Accounts, roles, and API keys | Existing identities, usable password hashes, and keys are retained where present in the recognized history; older histories receive the required later schema/defaults | Verify account access, locks, roles, and owner isolation; sign in again rather than expecting old browser cookies to transfer |
| Groups and recipients | IDs, ownership, target fields, and group membership are carried through | Compare saved membership and campaign recipient identity separately |
| Templates, pages, and attachments | Stored resource content and relationships are retained | Preview actual templates and supported attachment formats; served-page restrictions still apply |
| Sending profiles | Existing profile settings and stored service credentials are carried through supported histories | Very old histories may not contain later credential fields; confirm authentication and TLS with a controlled send |
| Campaigns and events | IDs, relationships, history, and supported timestamps are retained/transformed; event provenance is marked legacy | Compare lifecycle state and historical counts using the new metric definitions |
| Captured participant details and location | Opaque legacy event details are cleared from active records; result IP/location fields are cleared/zeroed | Preserve sensitive recovery copies separately; do not expect captured form values in exports |
| Reporting settings | Legacy IMAP settings become named mailbox resources; stored credentials are retained; invalid/incomplete connection settings can import disabled, with bounded defaults for port/folder/frequency | Review every imported mailbox and its first-poll checkpoint before enabling processing |
| Webhooks | Existing endpoints, activity flags, and secrets receive revision state | Validate the receiver and deduplication behavior; imported historical events do not imply a historical webhook replay |
| Campaign origins | Eligible-format HTTPS DNS origins are extracted as unverified domain records | Publish the new proof and verify routing/TLS; origin extraction does not confer trust |
| Files and process settings | Database migration does not transfer configuration files, certificates/private keys, static files, or service-manager settings | Copy and review them separately, using the target configuration contract |

Display preferences and persistent campaign drafts are Ruphish additions. A Gophish database does not supply equivalent saved UI state. Individual resource imports are separate from whole-database migration and can create duplicate work; they are not an idempotent substitute for this procedure.

Compare account IDs, resource ownership, group membership, templates, attachments, profiles, campaign relationships, historical events, and timestamps. Verify that existing account hashes authenticate without resetting every account. Keep exported profile data and integration credentials private during inspection.

Legacy participant-value capture is outside the Ruphish data model. Inspect the [privacy changes](differences-from-gophish.md) and resulting history representation rather than expecting arbitrary captured fields to remain accessible. Protect the pre-migration backup as sensitive legacy data.

Run your actual templates through preview and a controlled inbox. Check HTML/plain-text alternatives, escaping, attachments, and links. Register and verify the intended campaign origins. Exercise the API methods used by your scripts and compare meaningful fields and outcomes.

Inspect imported scheduled/active campaign and delivery states before allowing any worker to send. Ambiguous legacy work must be reconciled explicitly; do not infer that every unsent-looking row is safe to resend. Check [delivery outcomes](../user-guide/campaigns/delivery.md) and [migration verification](verification-and-rollback.md).

### Pending messages

Recognized legacy queue rows map by recipient, campaign, and owner. Completed campaigns become cancelled work. Rows marked as processing become `unknown`, with diagnostic `legacy_delivery_outcome_unknown`, because migration cannot establish whether SMTP accepted them. Otherwise, a row can become ready, or blocked with `campaign_page_missing` when its required page/policy is unavailable. Due times and supported retry counts carry across.

Ambiguous associations, duplicate queue identities, invalid attempt counts, or invalid due times are retained as legacy queue issues instead of guessed into runnable work. There is no documented bulk repair endpoint for those issues. Keep that campaign stopped, preserve the original, and investigate its mappings before resuming or recreating work.

### Links in already-sent messages

Preserved recipient tracking IDs alone are not enough. For a supported existing HTTPS link to work, retain its hostname, port, path/query and recipient identity, route it to Ruphish, and successfully verify the imported origin. The campaign must remain active, its page/policy must resolve, and the domain must remain enabled with accepted verification evidence.

Legacy plain-HTTP URLs, IP-address URLs, or unsupported origins are not automatically converted or authorized. Ruphish cannot rewrite a URL already inside a delivered message. Do not promise uninterrupted old-link behavior for those campaigns; record the limitation at cutover and use a verified HTTPS origin for future campaigns. Completed campaigns no longer serve normal participant interactions.

## Cut over

After a successful rehearsal, schedule a write stop, take a fresh consistent backup, and repeat the verified upgrade on the final target. Ensure only one application owns the database and external work. Verify sign-in, origin routing, a controlled send, reporting, and integration state before allowing normal campaigns.

Retain the rollback environment and record when new work begins. Rolling back after Ruphish has sent messages or processed reports requires reconciliation of those external effects; restoring a database cannot undo them.
