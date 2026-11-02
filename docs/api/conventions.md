---
title: API conventions
description: Understand JSON bodies, resource identities, revisions, timestamps, and pagination.
sidebar:
  order: 3
---

Send JSON writes with `Content-Type: application/json` unless an endpoint specifies a form or multipart body. Successful creation of groups, templates, pages, sending profiles, campaigns, domains, webhooks, and reporting mailboxes returns HTTP `201`. Reads, updates, deletions, preparation, draft saves/launches, and account creation normally return `200`; queued domain checks and mailbox polls return `202`. Consult the operation before hard-coding one success status.

## Paths and identifiers

Use the documented trailing slash on collection routes, such as `/api/templates/`. Some legacy slashless routes redirect with `301`; clients can change a redirected POST into a GET, so use the correct path directly.

Most resource IDs are integers. Draft IDs are client-selected strings. A recipient's opaque tracking identifier (`rid`, or `id` in a legacy result) is different from its numeric result-row ID in analysis and delivery APIs. Do not substitute one for the other.

## Updating a resource

Group, template, landing-page, learning-page, and SMTP `PUT` requests replace the resource's editable fields; they are not JSON Merge Patch. Include the resource `id` matching the path. Read the current object before editing it, preserve the fields you intend to keep, and send the response's `ETag` as `If-Match`.

These resource families return a numeric `revision` in details and summaries, and a quoted revision such as `ETag: "3"` on detail reads and saves. New and migrated resources start at revision 1; every successful configuration save advances it. Both PUT and DELETE require exactly one quoted numeric `If-Match`, for example `If-Match: "3"`. Revisions identify the current configuration; they do not retain version history. Timestamps are display metadata, not concurrency tokens. Wildcards, weak validators and lists of validators are rejected.

Domains, webhooks and reporting mailboxes require the numeric `revision` in update bodies and a quoted `If-Match` on deletion. Drafts carry their revision in save/launch bodies and the delete query parameter. Their existing identical-save retry acknowledgement remains idempotent; it does not merge different edits.

Missing preconditions return `428 REVISION_REQUIRED`; malformed `If-Match` returns `400 REVISION_REQUIRED`; a stale revision returns `412 RESOURCE_CHANGED`; a resource removed before the write returns `404 RESOURCE_NOT_FOUND`. Revision checking, replacement/deletion and edit attribution happen in one database transaction. Of two competing writes against the same version, only one can commit. Losing edits never recreate deleted resources. These requirements apply to API clients as well as the UI; unconditional legacy updates are no longer supported.

Keep the user's input on a conflict. Read the latest resource separately, compare changes and obtain an explicit decision before replacing their draft or submitting against a newer revision. Never automatically retry with the latest revision. The UI offers **Review saved version in a new tab** and a confirmed **Discard my changes and reload** action. If the record was deleted, copy or export anything needed before leaving.

Resource names must be unique for new resources and renames within their resource family across the installation. Existing imported duplicate names may remain editable. Prefer explicit IDs and revisions when a workflow supports them.

## Time and missing values

Resource timestamps use UTC RFC3339 strings, with explicit `null` for absent or unknown dates. An account with no recorded login likewise returns `last_login: null`; the year-0001 sentinel is not an API representation of absence. New analysis, domain, integration, and delivery fields may use UTC Unix seconds; each reference identifies them. Local scheduling inputs pair a local date/time with an IANA time-zone identifier. Do not apply the display locale's time zone to an API timestamp.

Resource `targets`, `attachments` and `headers` are arrays and return `[]` when empty, including immediately after saving. Collection inputs accept arrays, not `null`. Campaign `results` and `timeline` can still be absent. Do not assume every collection is always an array. New paginated APIs expose their continuation field explicitly.

## Pagination and retries

Legacy resource lists return their collection without a shared pagination envelope. Drafts, the campaign directory, delivery, and analysis use different paging schemes. Follow each endpoint's cursor or offset contract; there is no universal `page` parameter.

Retry reads after transient transport errors with a bounded delay. Do not automatically retry a write that might have succeeded. Draft launch provides identity-based retry behavior; direct campaign creation supports an `Idempotency-Key` header. Preserve the same operation identity and request when retrying a launch. Test sends and SMTP delivery can have an uncertain outcome.

See [errors](errors.md) for status handling and [campaigns](campaigns.md) for delivery resolution.
