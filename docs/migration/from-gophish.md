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
4. Build the target Ruphish checkout and prepare its current UI distribution. Translate configuration using the [configuration reference](../administration/configuration.md), including TLS trust, database credentials, and origin verification settings. Remove `migrations_prefix`; the release accepts only embedded migrations.
5. Start Ruphish against the copy and inspect startup logs. An unrecognized or inconsistent history is a stop condition, not a reason to clear migration tables or create an empty replacement database.

SQLite creates a verified snapshot before upgrading supported legacy state. MySQL uses staged tables and retained originals with an upgrade journal. These protections supplement your own backup; preserve the original until verification is complete.

## Verify data and workflows

### What carries across

The same-engine upgrade converts a recognized Gophish database in place to Ruphish application schema v1. It preserves supported object IDs and relationships while applying the transformations below. Verify the result against your database copy before cutover.

| Data | Upgrade treatment | Follow-up |
| --- | --- | --- |
| Accounts, roles, and API keys | Existing identities, usable password hashes, and keys are retained where present in the recognized history; older histories receive the required later schema/defaults | Verify account access, locks, roles, and owner isolation; sign in again rather than expecting old browser cookies to transfer |
| Groups and recipients | IDs, ownership, target fields, and group membership are carried through | Compare saved membership and campaign recipient identity separately |
| Templates, pages, and attachments | Stored resource content and relationships are retained | Preview actual templates and supported attachment formats; served-page restrictions still apply |
| Sending profiles | Existing profile settings and stored service credentials are carried through supported histories | Very old histories may not contain later credential fields; confirm authentication and TLS with a controlled send |
| Campaigns and events | IDs, relationships, history, and supported timestamps are retained/transformed; event provenance is marked legacy | Compare lifecycle state and historical counts using the new metric definitions |
| Captured participant details and location | Opaque legacy event details are cleared from active records; result IP/location fields are cleared/zeroed | Preserve sensitive recovery copies separately; do not expect captured form values in exports |
| Reporting settings | Legacy IMAP settings become named mailbox resources; stored credentials are retained; invalid/incomplete connection settings can import disabled, with bounded defaults for port/folder/frequency | Review every imported mailbox before enabling processing; the first successful poll checkpoints existing mail and watches new arrivals |
| Webhooks | Existing endpoints, activity flags, and secrets receive revision state and an owner; see [webhook ownership](#webhook-ownership) | Use the assigned owner or explicit impersonation to review them; imported historical events do not imply a historical webhook replay |
| Campaign origins | Fixed HTTP/HTTPS DNS and IP origins are extracted without verification evidence | Enable the origin and satisfy the current verification policy; origin extraction does not confer trust |
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

Preserved recipient tracking IDs alone are not enough. For a supported existing HTTP/HTTPS link to work, retain its scheme, hostname or IP address, port, path/query and recipient identity, and route it to Ruphish. The campaign must remain active, its page/policy must resolve, and the domain must remain enabled. When verification is required, the origin must be a DNS hostname over HTTPS with accepted verification evidence.

HTTP and IP origins remain unavailable when verification is required. Dynamic hosts and invalid origins remain unsupported. Ruphish cannot rewrite a URL already inside a delivered message; check the origin and policy during cutover rather than promising uninterrupted old-link behavior. Completed campaigns no longer serve normal participant interactions.

## Cut over

After a successful rehearsal, schedule a write stop, take a fresh consistent backup, and repeat the verified upgrade on the final target. Ensure only one application owns the database and external work. Verify sign-in, origin routing, a controlled send, reporting, and integration state before allowing normal campaigns.

Retain the rollback environment and record when new work begins. Rolling back after Ruphish has sent messages or processed reports requires reconciliation of those external effects; restoring a database cannot undo them.

## Webhook ownership

Gophish webhooks do not record a creator. Conversion assigns them to the existing account with the lowest user ID whose role grants `modify_system`; this is migration ownership, not a reconstructed creator identity. URLs, signing secrets, and individual activation states are preserved. Conversion rejects a source containing webhooks but no such administrator. New webhook ownership is always taken from the authenticated creator. Review migrated endpoints through that account or explicit administrator impersonation; delivery still includes events from all campaign creators.

## Email-open tracking

Migrated campaigns default to `track_opens: true`, regardless of whether the stored Gophish template contains a tracking image. Gophish has no separate Boolean for its template checkbox; the HTML is preserved. This default governs Ruphish behavior and does not prove that historical Gophish emails contained a pixel. New campaigns also default to enabled and can disable **Track email opens** before launch.

## Browser and device history

Migrated events have no collected browser, OS, or device categories (`client: null`). New click and submission requests collect only the coarse categories described in [participant data](../user-guide/participant-data.md#browser-and-device-observations).
