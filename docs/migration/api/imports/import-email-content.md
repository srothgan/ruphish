---
title: Import email content
description: Migrate POST /api/import/email to the current Ruphish contract.
sidebar:
  order: 181
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Retain the import operation while adapting validation and response handling.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/import/email` | `POST /api/import/email` |
| Success | `200 OK` | `200 OK` |
| Response type | `emailResponse` | `resources::EmailImport` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `content` | Raw RFC 822/MIME message string. | Same type; 8 MiB content bound. |
| `convert_links` | Boolean replacing HTML anchor destinations with the campaign template URL. | Same request field, default false. Preview transformed content before saving. |
| Response | `subject`, `text`, `html` strings. | Same fields. This response is not a saved template and does not include an attachment collection. |

## Gophish request

`POST /api/import/email`

```json
{
  "content": "Subject: Training notification\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=example\r\n\r\n--example\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello Ada\r\n--example\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello Ada</p>\r\n--example--\r\n",
  "convert_links": false
}
```

## Gophish response

```json
{
  "subject": "Training notification",
  "text": "Hello Ada",
  "html": "<p>Hello Ada</p>"
}
```

## Ruphish request

Request type: `resources::request::EmailRequest`. Save this body as `request.json`:

```json
{
  "content": "Subject: Training notification\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=example\r\n\r\n--example\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello Ada\r\n--example\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello Ada</p>\r\n--example--\r\n",
  "convert_links": false
}
```

```sh
curl --request POST "$RUPHISH_URL/api/import/email" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

## Ruphish response

```json
{
  "subject": "Training notification",
  "text": "Hello Ada",
  "html": "<p>Hello Ada</p>"
}
```

## Behavior and verification

Requires `modify_objects`. Import returns content only. Save it with the email-template API and verify the current template renderer and any separately managed attachments.

Test representative valid content and a malformed input in isolation. Inspect the returned content before saving it as a shared resource.

Current contract: [Import email content](../../../api/imports-previews-and-test-sends.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/import.go).
