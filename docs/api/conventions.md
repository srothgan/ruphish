---
title: Conventions
description: Wire formats, resource metadata, revisions, timestamps, and retry contracts.
sidebar:
  order: 2
---

Send a Bearer API key on every request to the Ruphish application origin. Requests and responses use UTF-8 JSON unless the endpoint specifies multipart data, form data, CSV, or binary output.

## Reading the examples

Examples show synthetic data with the current request and response shapes. Replace IDs, domains, keys, dates, and credentials with values from your installation. Examples illustrate individual operations; they are not a script to execute consecutively. For JSON requests, save the shown body as `request.json` before running its curl command. A request described as “No body” sends neither `{}` nor `null`.

Type names such as `campaign::Create` identify the current contract type. JSON field names, nullability, and enum values in the examples and tables are the wire contract; Rust types are not required to use the service. Integer fields are JSON numbers, with signed 64-bit range unless a narrower bound is listed.

## Paths and collections

Use the documented trailing slash for resource collections. GET and POST on `/api/groups`, `/api/templates`, `/api/pages`, `/api/learning-pages`, `/api/smtp`, `/api/campaigns`, and `/api/users` redirect to their slash form with `301`. Send writes directly to the canonical slash URL to avoid client-specific redirect behavior. Other routes do not automatically share those aliases.

Collections return arrays or named page objects as shown. An empty collection is `[]`, not null. Pagination is endpoint-specific: follow the returned cursor and keep the selection and ordering unchanged.

## Attribution

The following fields are flattened into resource, campaign, draft, domain, and integration responses:

```json
{
  "created_at": "2026-11-03T12:00:00Z",
  "created_by": {
    "id": 1,
    "username": "operator",
    "deleted": false
  },
  "edited_by": {
    "id": 1,
    "username": "operator",
    "deleted": false
  },
  "edited_at": "2026-11-03T12:00:00Z"
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `created_at` | `string or null` | UTC creation instant, or unknown when historical evidence is unavailable. |
| `created_by` | `UserReference or null` | Original creator, or unknown for unattributed historical resources. |
| `edited_by` | `UserReference or null` | Last authenticated editor, or unknown for imported history. |
| `edited_at` | `string or null` | UTC timestamp of the successful save. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Stable account identifier, retained after deletion. |
| `username` | `string or null` | Display name, or unknown when historical identity information is unavailable. |
| `deleted` | `boolean` | True when a retained deleted-account record confirms deletion. |

Each attribution member is nullable. A historical account ID may have `username: null`; deleted accounts use `deleted: true`. The server assigns these fields. They are not write inputs.

## Resource metadata

Groups, templates, landing pages, learning pages, and sending profiles combine attribution with this metadata and their content fields:

```json
{
  "created_at": "2026-11-03T12:00:00Z",
  "created_by": {
    "id": 1,
    "username": "operator",
    "deleted": false
  },
  "edited_by": {
    "id": 1,
    "username": "operator",
    "deleted": false
  },
  "edited_at": "2026-11-03T12:00:00Z",
  "revision": 1,
  "id": 1,
  "name": "Example resource",
  "modified_date": "2026-11-03T12:00:00Z"
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `revision` | `integer` | Monotonic configuration revision used for conditional writes. |
| `id` | `integer` | Persistent resource identity. |
| `name` | `string` | Display name. |
| `modified_date` | `string or null` | Last modification, or explicitly unknown. |

## Updating a resource

Conditional writes prevent silently replacing a newer version. Read the latest revision, submit the appropriate precondition, and retain the new revision returned by a successful save.

| Resource | Replacement precondition | Deletion precondition |
| --- | --- | --- |
| Groups, templates, pages, learning pages, SMTP | `If-Match: "1"` | `If-Match: "1"` |
| Domains, webhooks, reporting mailboxes | JSON `"revision": 1` | `If-Match: "1"` |
| Campaign drafts | JSON `"revision": 1` | Query `?revision=1` |
| Company settings | `If-Match: "1"` | No delete operation |
| Campaign creation selections | String `"revision": "1"` on each selected resource | Not applicable |

Resource If-Match values are a single quoted positive numeric revision. Weak ETags, wildcards, multiple values, unquoted values, and noncanonical numbers are rejected. Missing required preconditions return `428`; a stale revision returns `412`. Settings has its own parser; use the same quoted format there.

PUT replaces the editable configuration rather than applying a JSON patch. Omission, null, and empty strings can have different meanings, especially for secrets. Follow the endpoint’s field table.

## Timestamps and identifiers

RFC 3339 timestamps are strings with an offset, normally normalized to UTC with `Z`. Operational timestamps explicitly documented as Unix seconds are integer seconds, not milliseconds. Nullable timestamps are returned as null when unknown.

Public recipient tokens (`rid`, or the string `id` in a full campaign recipient) differ from numeric recipient row IDs in results and delivery endpoints. Resource IDs are scoped to their resource family.

## Success and retry behavior

There is no universal success envelope. A resource read returns its object directly. Some actions instead return:

```json
{"success":true,"message":"Delivery updated","data":null}
```

`success` is boolean, `message` is diagnostic text, and `data` is the operation-specific value or null. Integration deletion returns only `{"success":true}`. `202 Accepted` means queued work, not a completed verification or mailbox check.

After an uncertain write response, reconcile using the operation’s identity or current resource state. Campaign and draft creation have explicit idempotency rules; a test email does not become safe to repeat simply because its HTTP response was lost.
