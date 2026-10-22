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

Group, template, landing-page, and SMTP `PUT` requests replace the resource's editable fields; they are not JSON Merge Patch. Include the resource `id` matching the path. Read the current object before editing it, preserve the fields you intend to keep, and send the response's `ETag` as `If-Match`.

For these resource families, `If-Match` is optional for legacy compatibility. A stale value returns `412`. Domains, drafts, and integrations have their own revision requirements described on their pages. A conflict means reload and reconcile; retrying the same stale write does not resolve it.

Resource names must be unique for new resources and renames within their owner and type. Existing imported duplicate names may remain editable. Prefer explicit IDs and revisions when a workflow supports them.

## Time and missing values

Legacy resource timestamps use timestamp strings. New analysis, domain, integration, and delivery fields may use UTC Unix seconds; each reference identifies them. Local scheduling inputs pair a local date/time with an IANA time-zone identifier. Do not apply the display locale's time zone to an API timestamp.

Some legacy lists serialize as `null` or omit fields when empty. In particular, campaign `results` and `timeline` can be absent. Do not assume every collection is always an array. New paginated APIs expose their continuation field explicitly.

## Pagination and retries

Legacy resource lists return their collection without a shared pagination envelope. Drafts, the campaign directory, delivery, and analysis use different paging schemes. Follow each endpoint's cursor or offset contract; there is no universal `page` parameter.

Retry reads after transient transport errors with a bounded delay. Do not automatically retry a write that might have succeeded. Draft launch provides identity-based retry behavior; direct campaign creation supports an `Idempotency-Key` header. Preserve the same operation identity and request when retrying a launch. Test sends and SMTP delivery can have an uncertain outcome.

See [errors](errors.md) for status handling and [campaigns](campaigns.md) for delivery resolution.
