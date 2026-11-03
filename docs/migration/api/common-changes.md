---
title: Common contract changes
description: Apply shared access, API-key, revision, error, and serialization changes across a migrated client.
sidebar:
  order: 61
prev: {"label": "Migration overview", "link": "/docs/migration/"}
next: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
---

Use this page for changes shared by multiple endpoints. The [endpoint inventory](index.md) identifies operation-specific differences. The comparison baseline is Gophish 0.12.1; Ruphish examples use the current released contract.

## API keys and permissions

Send the account's API key in the Authorization header on every integration request:

```http
Authorization: Bearer <api-key>
```

Existing keys are retained by supported database conversion. A retained key does not bypass account locks, required password setup, or endpoint permissions. There is no need to implement the built-in UI's sign-in flow for an API-key integration. See [Authentication](../../api/authentication.md).

Gophish clients often used an `api_key` query parameter. Ruphish still accepts that parameter, but the examples use the Bearer header so URLs do not carry credentials. Remove a stale query credential when switching to headers: an explicit query key takes precedence over the Authorization header.

Resources and campaigns are shared across the installation. `created_by` and `edited_by` describe attribution; they do not filter results to the caller. Review any integration that assumed a separate dataset for each API key. Reads generally require `view_objects`, edits `modify_objects`, and company/account administration `modify_system`; endpoint pages identify exceptions.

## Reading the examples

Examples contain synthetic values and illustrate independent operations. They are not successive commands or captured production responses. Replace keys, IDs, domains, dates, and credentials with your rehearsal values. A resource created during a new Ruphish example can have full attribution; a migrated record can instead contain null attribution. Attribute order in JSON is not significant.

The curl examples use a POSIX shell with `RUPHISH_URL` set to your Ruphish application origin and `RUPHISH_API_KEY` supplied through your environment. Use the URL without a trailing slash. For JSON requests, save the shown Ruphish body as `request.json`. These are API calls to the application listener, not the campaign-page listener. Use your trusted HTTPS origin when connecting across a network.

Request and response type names identify the current contract; you do not need Rust to implement a client. Field tables identify breaking differences. The linked HTTP API reference owns the complete current schema, constraints, and new operations.

## Serialization and metadata

| Area | Gophish baseline | Ruphish requirement |
| --- | --- | --- |
| Unknown timestamps | Go zero-time strings can appear. | Accept null where the current type is nullable. Operational timestamps can instead use integer Unix seconds; read the endpoint's units. |
| Attribution | Not present in ordinary resource responses. | Nullable creation/edit timestamps and user references are added. Unknown/deleted historical identities need explicit handling. |
| Secrets | SMTP passwords, IMAP passwords, and webhook secrets can be returned. | Those resource reads return presence flags instead. Account endpoints still return API keys to authorized callers. |
| Request models | Many endpoints decode directly into persisted Go models. | Construct writable request objects. Campaign and integration commands reject unknown fields; ordinary resource writes ignore them. Neither behavior makes response fields writable. |
| Recipient IDs | Full campaign results expose a string public token. | That token remains in full campaign details; analysis and delivery use separate numeric row IDs. |
| Success envelopes | Resources and action envelopes differ. | Continue parsing by operation. Webhook/mailbox deletion returns only `success`; there is no universal response wrapper. |

Use [current conventions](../../api/conventions.md) for exact nullable fields and metadata types. Do not coerce null timestamps to an arbitrary current time or convert public tracking tokens into integers.

## Conditional writes

| Operation | Required precondition |
| --- | --- |
| PUT/DELETE group, template, landing page, or sending profile | Quoted current revision in `If-Match`, for example `If-Match: "1"`. |
| PUT webhook or reporting mailbox | Integer `revision` in the JSON body. |
| DELETE webhook or reporting mailbox | Quoted current revision in `If-Match`. |
| Create a reviewed campaign | Selected resource revisions are strings inside `selection`; use the versions returned by preparation. |

A missing required edit/delete precondition returns `428`; a stale one returns `412`. Read the latest object and resolve the conflict before retrying. Do not automatically fetch a new version and overwrite another user's change without review.

An example Ruphish conflict response:

```json
{
  "success": false,
  "message": "This resource changed. Your request was not applied. Review the current version before trying again.",
  "data": null,
  "code": "RESOURCE_CHANGED"
}
```

## Errors, retries, and methods

Gophish application errors commonly returned `success`, `message`, and `data`. Ruphish adds a stable string `code`, and some validation failures include `validation.issues` with field paths and reason codes. Use HTTP status and `code` for control flow; do not parse English diagnostic text. Allow unknown future codes. See [Errors](../../api/errors.md).

Use canonical paths and explicit methods. In particular, campaign completion is now POST. Sending a write to a collection URL that redirects can change behavior in some HTTP clients; use the documented trailing slash directly.

Retries are operation-specific. Creation of a campaign has an optional `Idempotency-Key`; resource creation and test sends do not acquire the same guarantee. After a lost response, reconcile the resource or operation identity before repeating a write. A successful HTTP response for a queueing action does not mean its asynchronous work finished.
