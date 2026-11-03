---
title: Webhook delivery changes
description: Adapt event receivers to sanitized payloads, stable identity, acknowledgements, and durable retries.
sidebar:
  order: 256
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update receivers as well as the webhook configuration client. The request remains a signed JSON POST to your configured URL, but acknowledgement and delivery behavior differ.

| Attribute / behavior | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| `campaign_id`, `email`, `time`, `message` | Integer campaign ID, recipient string, RFC 3339 timestamp string, and event label string. | Same outer names/types for ordinary event delivery. |
| `details` | String-encoded details could include captured values and raw browser/network information. | Sanitized string-encoded details, or empty string. No participant-entered values. It remains a string, not an outer JSON object. |
| `client` | Absent. | Object with coarse `browser`, `os`, `device` categories, or null when not available. |
| Event identity | No stable delivery ID header in the pinned sender. | `X-Ruphish-Event-ID`; deduplicate by source installation plus this ID. |
| Signature | `X-Gophish-Signature: sha256=<hex>` over the exact request body. | Same header and HMAC-SHA256 construction. Verify the raw bytes before decoding. |
| Receiver response | Pinned sender treated HTTP statuses below 400 as success and did not follow redirects. | Acknowledge with 2xx; redirects are not followed or accepted as successful delivery. Response body is discarded. |
| Delivery failure | Fire-and-forget sender with no durable retry queue. | Durable attempts; selected network/HTTP failures retry. Duplicates and out-of-order arrival are possible. |
| Configuration change | No revision-bound queue. | Queued work for the old revision is cancelled; an in-flight request can still complete. |

## Gophish event request

POST to the receiver with `Content-Type: application/json` and the signature header:

```json
{
  "campaign_id": 1,
  "email": "ada@example.com",
  "time": "2026-11-03T12:00:00Z",
  "message": "Clicked Link",
  "details": "{\"payload\":{},\"browser\":{\"address\":\"192.0.2.10\",\"user-agent\":\"Example browser\"}}"
}
```

## Ruphish event request

POST to the receiver with `Content-Type: application/json`, the signature header, and `X-Ruphish-Event-ID`:

```json
{
  "campaign_id": 1,
  "email": "ada@example.com",
  "time": "2026-11-03T12:00:00Z",
  "message": "Clicked Link",
  "details": "",
  "client": {
    "browser": "firefox",
    "os": "linux",
    "device": "desktop"
  }
}
```

Example acknowledgement, with no response body:

```http
HTTP/1.1 204 No Content
```

The JSON examples are readable representations, not bytes to reuse with a previously computed signature. Whitespace and serialization changes alter the HMAC input.

## Validation and retries

Configuration validation sends a different body:

```json
{"success": true}
```

Accept that validation payload without attempting to parse it as a campaign event. Validation success does not prove that your event parser or deduplication logic works.

Ruphish uses a ten-second request deadline. Network failures and HTTP 408, 429, 500, 502, 503, and 504 can be retried. Persist accepted event processing before returning success: a timeout after your receiver accepted an event can cause another delivery. Do not derive event order from arrival order or deduplicate solely by recipient and message label; repeated observations are valid distinct events.

Test a signed event, a repeated delivery with the same event ID, a new event with the same recipient/label, and a validation request. See [Webhooks](../../api/webhooks.md) for the current contract and [the pinned Gophish sender](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/webhook/webhook.go) for the comparison baseline.
