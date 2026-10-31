---
title: Webhooks API and event contract
description: Manage endpoints, verify signed payloads, and handle retries and duplicate events.
sidebar:
  order: 17
---

Webhooks deliver committed campaign events to administrator-configured endpoints. They can span resource owners, so authorize receivers for that installation-wide scope.

| Method | Path | Purpose |
| --- | --- | --- |
| GET, POST | `/api/webhooks/` | List or create endpoints |
| GET, PUT, DELETE | `/api/webhooks/{id}` | Read, edit, or delete |
| POST | `/api/webhooks/{id}/validate` | Send a signed validation request |

Writes contain `name`, `url`, optional `secret`, `is_active`, and optional `revision`. Send the current revision on edits and `If-Match` on deletion. Responses include identity, configuration, revision, `has_secret`, and recent `history`; they never return the secret. Omitting a secret on edit preserves it. Use HTTPS for a receiving service.

## Event payload

The JSON body includes the event fields and optional coarse client observation. This is a synthetic example, not a captured response:

```json
{
  "campaign_id": 42,
  "email": "alex@example.test",
  "time": "2026-10-22T10:00:00Z",
  "message": "Clicked Link",
  "details": "{}",
  "client": { "browser": "firefox", "os": "linux", "device": "desktop" }
}
```

`details` is a string containing sanitized event details, not a nested JSON object in the outer envelope. Do not expect arbitrary participant form data. Validation sends `{"success":true}` instead of a campaign event.

Event payloads also contain nullable `client` with the same closed `browser`, `os`, and `device` categories as [result events](analysis-and-exports.md#client-observations). Click and submission requests carry these categories; historical events, opens, and reports have no collected client metadata. Raw User-Agent headers, versions, device models, and network identifiers are excluded.

`X-Ruphish-Event-ID` identifies an ordinary event consistently across its retry attempts. Treat it as opaque and scope stored identities to the source installation. A receiver should persist deduplication state before acknowledging successful processing.

## Verify the signature

`X-Gophish-Signature` contains `sha256=` followed by lowercase hexadecimal HMAC-SHA256. Calculate it over the exact received body bytes, before parsing or reserializing JSON, using the configured secret.

The verification algorithm is language-independent. In pseudocode:

```text
body = exact received request body bytes
signature = value of the X-Gophish-Signature header
secret = configured webhook secret

if signature is missing or contains non-ASCII characters:
    reject the request

digest = HMAC-SHA256(key = UTF8(secret), message = body)
expected = "sha256=" + lowercase_hex(digest)

if not constant_time_equal(ASCII(expected), ASCII(signature)):
    reject the request

continue with event deduplication and processing
```

Reject an invalid signature before processing. Use your language's cryptographic library for HMAC and constant-time comparison. Signature verification establishes authenticity; it does not provide replay prevention. Event-ID deduplication supplies that separate responsibility.

## Delivery behavior

Ruphish persists delivery work and bounds concurrent webhook requests to four. Each request has a ten-second deadline. Redirects are not followed. Any 2xx response counts as success; response bodies are discarded.

Network failures and statuses 408, 429, 500, 502, 503, and 504 are retryable, up to eight total attempts. Delays grow exponentially from a 30-second base, cap at one hour, and include jitter. Other HTTP failures are terminal for that delivery.

There is no ordering or exactly-once guarantee. A timeout can happen after the receiver processed the event. Return success only after durable acceptance, and tolerate duplicates and out-of-order arrivals.

Editing an endpoint cancels queued work for its old configuration. An already authorized or in-flight request can still finish. During secret rotation, allow for this overlap at the receiver; retiring the old secret immediately can reject those requests. See [administrative operation](../administration/webhooks.md) for validation and history.
