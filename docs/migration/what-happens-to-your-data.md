---
title: What happens to your data
description: Understand preserved records, transformed history, shared access, and pending delivery after conversion.
sidebar:
  order: 20
---

Conversion preserves supported identities and relationships while applying Ruphish's data model. Use this reference when comparing the rehearsal database with the source. The same rules apply to SQLite and MySQL unless stated otherwise.

| Data | Conversion treatment | What to verify |
| --- | --- | --- |
| Accounts and roles | Existing account IDs, role assignments, lock/reset flags, supported password hashes, and API keys are retained where present in the source history. Earlier schemas receive later required fields/defaults. | Existing account access, role permissions, and locks. Do not reset every account merely because the application changed. |
| Access to resources and campaigns | Historical creator IDs become attribution; access uses shared installation permissions. | Users and API clients may now see objects created by other accounts. Attribution is not ownership isolation. |
| Groups and recipients | Resource IDs, recipient attributes, membership, and campaign recipient relationships are retained. | Compare group membership separately from historical campaign audiences; they can legitimately differ. |
| Templates, pages, attachments | Stored content and supported relationships are retained; serving/rendering follows Ruphish rules. | Preview subjects, text/HTML, attachments, links, scripts, and forms. |
| SMTP profiles | Stored connection settings and credentials are retained where the source schema provides them. | A controlled connection and send using current TLS/authentication rules. Passwords are no longer returned by resource reads. |
| Campaigns | IDs, recipient identities, schedules, lifecycle information, and available content are retained/transformed. | Active versus completed state, schedules, missing content, public links, and delivery disposition. |
| Events | Known event names map to Ruphish kinds; imported provenance is `legacy`. Unknown historical names remain distinguishable. | Event counts, timestamps, recipient associations, and the new count definitions. |
| Opaque event details and location | Legacy detail payloads are cleared in active data; result IP is empty and latitude/longitude are zero. | Consumers no longer retrieve captured form values or historical geolocation from the active application. |
| Browser/device observations | No categories are reconstructed for imported events: `client` is null. | Distinguish missing historical evidence from a newly observed client category. |
| IMAP settings | Legacy rows become named shared reporting mailboxes. Credentials are preserved; incomplete settings or missing/invalid/deleted creators cause disabled imports for review. | Every mailbox's enablement, connection fields, attribution, and first processing boundary. |
| Webhooks | URLs, activation flags, and secrets are retained. Revision starts at 1; creator/editor attribution is unknown because Gophish did not record it. | Feature availability, receiver configuration, and new delivery behavior. |
| Campaign origins | Supported fixed HTTP/HTTPS DNS or IP origins are extracted from campaign URLs without proof of domain ownership. | Enabled state, current eligibility, routing, and verification policy. |
| Configuration and external files | Not copied by database conversion. | Explicit process configuration, certificates/private keys, external assets, and service settings. |

Resource revisions begin in Ruphish; they are not reconstructed Gophish edit history. Missing historical attribution remains null rather than inventing an editor. Source allocation counters are preserved so migration does not reuse IDs solely because higher-numbered rows were deleted.

## Campaign content

Gophish did not retain launch-time resource versions. Imported campaigns freeze the template, attachments, landing page, and sender identity/headers available at conversion. This is a conversion-time record, not proof of the content in the original delivered message. Missing resources remain explicitly unavailable.

New Ruphish campaigns snapshot selected recipients and authored content at launch. SMTP connection settings and credentials remain live for delivery/recovery; a content snapshot does not freeze a password. See [Campaigns](../user-guide/campaigns/index.md).

Review saved form fields before reusing a landing page. Gophish could remove input `name` attributes according to its capture settings; conversion preserves that saved HTML and does not restore removed attributes. Ruphish's password-presence observation requires a named, enabled, nonempty password field. Restore intended field names in the authored page and test it before launching a new campaign; Ruphish does not collect the entered password value.

Migrated campaigns default to `track_opens: true`. The stored HTML is preserved; this setting does not establish whether an old Gophish message included a tracking image.

## Pending messages

Queue conversion resolves each row against its recipient, campaign, and historical creator identity. The creator account does not have to remain present for a valid association.

| Source condition | Converted state | Action |
| --- | --- | --- |
| Valid queue row in an already completed campaign | `cancelled` | Keep closed work closed. |
| Valid queue row marked as processing | `unknown`, code `legacy_delivery_outcome_unknown` | Check SMTP evidence before confirming acceptance or authorizing a resend. Migration cannot determine whether the server accepted it. |
| Valid pending row with required campaign page/policy available | `ready` | Verify the due time and recipient before allowing the queue worker to run. |
| Valid pending row missing required page/policy | `blocked`, code `campaign_page_missing` | Inspect the campaign's missing content before recovery. |
| Ambiguous/missing associations, duplicate queue identities, invalid retry count, or invalid due time | Retained legacy queue issue; excluded from runnable delivery | Preserve the source and investigate. No bulk repair endpoint translates these issues into valid work. |

Supported retry counts and due times carry across. Do not assume every recipient without a sent status is safe to resend. Use [Delivery and recovery](../user-guide/campaigns/monitor-sending.md), or the [delivery endpoints](../api/delivery-and-history.md), for supported recovery decisions.

## Existing campaign links

Recipient tracking IDs are preserved, but links also depend on their original URL and campaign state. Retain the original scheme, hostname/IP, port, path, and query parameters, and route requests to Ruphish's campaign listener. The campaign must be active, its page/policy available, and its domain enabled and eligible under the current verification policy.

When verification is required, an eligible origin must use HTTPS and a DNS hostname with successful verification. HTTP/IP origins cannot meet that policy. Review [Domain setup](../user-guide/resources/domains.md) and [Network access and HTTPS](../administration/hosting-and-maintenance/network-access-and-https.md). Ruphish cannot change URLs already delivered in email. Test an existing link during rehearsal with controlled routing; do not promise continuity from preserved IDs alone.

## Reporting mailboxes

Imports supply bounded defaults where legacy values are missing or invalid: port `993`, folder `INBOX`, and frequency `60` seconds. A frequency already in the supported `30-31540000` range is retained. A missing host, username, or password, an invalid port, or an unresolved creator disables the imported mailbox for review. Enablement does not prove that its credentials or connection work. Positive historical creator IDs remain attribution; invalid IDs become unknown.

Conversion removes these rows from active legacy `imap` storage after creating the mailbox resources. Their originals remain in the SQLite recovery snapshot or MySQL `rp_original_imap`, as well as your own backup.

The initial processing boundary considers the earliest actual Ruphish send in active campaigns across creators; without an eligible send, polling starts with new arrivals. Do not assume the first poll replays the entire historical inbox. One report must identify a single distinct recipient-ID/origin pair; ambiguous bundled reports are rejected. Reports received after campaign completion do not change its results. See [Reporting mailboxes](../user-guide/resources/reporting-mailboxes.md).

## Webhooks and retained recovery data

Imported webhooks need no administrator adoption step. Accounts with the resource permissions manage shared configurations once the administrator enables the feature. Delivery includes events across campaigns; conversion does not replay all imported history to receivers. See [Webhook delivery changes](api/webhook-delivery-changes.md).

Conversion sets global mailbox/webhook feature availability when corresponding configurations are imported. Individual enablement still matters. This is why rehearsal isolation must be established before the first start.

SQLite snapshots, MySQL retained originals, and independent backups can still contain captured participant values and credentials removed from active records. Apply your backup access and retention policy to those copies; sanitizing the active database does not sanitize its backups.
