---
title: Send a test email
description: Migrate POST /api/util/send_test_email to the current Ruphish contract.
sidebar:
  order: 190
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Replace implicit SMTP name/inline detection with an explicit saved or draft profile selection.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/util/send_test_email` | `POST /api/util/send_test_email` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<()>` |

Use a Bearer API key. The [common changes](common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `smtp` | Full SMTP object, falling back to lookup by `name`. | Tagged object: `{"source":"saved","id":1}` or `{"source":"draft","profile":{...}}`. |
| `template.name`, `page.name` | Saved-resource names. | Still names in this utility endpoint; do not confuse them with campaign creation's ID selections. Ambiguous shared names must be resolved. |
| `url`, `email`, `first_name`, `last_name`, `position` | Strings. | Retained for this test destination and personalization. |
| Response | 200 envelope with `Email Sent`. | Same envelope, including nullable `data`. Success is not proof of inbox delivery. |

## Gophish request

`POST /api/util/send_test_email`

```json
{
  "template": {
    "name": "Training message"
  },
  "page": {
    "name": "Training page"
  },
  "smtp": {
    "name": "Training sender"
  },
  "url": "http://127.0.0.1:8081",
  "email": "ada@example.com",
  "first_name": "Ada",
  "last_name": "Lovelace",
  "position": "Engineer"
}
```

## Gophish response

```json
{
  "success": true,
  "message": "Email Sent",
  "data": null
}
```

## Ruphish request

Request type: `resources::request::TestRequest`. Save this body as `request.json`:

```json
{
  "template": {
    "name": "Training message"
  },
  "page": {
    "name": "Training page"
  },
  "smtp": {
    "source": "saved",
    "id": 1
  },
  "url": "http://127.0.0.1:8081",
  "email": "ada@example.com",
  "first_name": "Ada",
  "last_name": "Lovelace",
  "position": "Engineer"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/util/send_test_email" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

## Ruphish response

```json
{
  "success": true,
  "message": "Email Sent",
  "data": null
}
```

## Behavior and verification

Requires `modify_objects`. The test performs a real send. This operation has no campaign-creation idempotency guarantee, so a lost response must not trigger an automatic resend. For a sample of a complete reviewed campaign, use the new campaign test endpoint linked from the current reference.

Use one controlled recipient and a saved profile; verify secret preservation separately if using a draft profile.

Current contract: [Send a test email](../../api/imports-and-previews.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/util.go).
