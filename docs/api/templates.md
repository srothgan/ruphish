---
title: Templates API
description: Manage email bodies, subjects, sender overrides, and attachments.
sidebar:
  order: 6
---

Templates provide personalized email content. They are separate from the SMTP configuration used to send it.

| Method | Path | Result |
| --- | --- | --- |
| GET, POST | `/api/templates/` | List or create templates |
| GET, PUT, DELETE | `/api/templates/{id}` | Read, replace, or delete a template |
| GET | `/api/templates/summary` | `total` and `items` with identity, modification time, and active campaign counts |

## Create a template

```json
{
  "name": "Local invitation",
  "subject": "Hello {{.FirstName}}",
  "text": "Open the local exercise: {{.URL}}",
  "html": "<p>Hello {{.FirstName}}</p><p><a href=\"{{.URL}}\">Open the exercise</a></p>{{.Tracker}}",
  "envelope_sender": "",
  "attachments": []
}
```

Send the body to `POST /api/templates/`. The response adds `id` and `modified_date`. A PUT must include the matching `id`; use the ETag for a conditional edit.

`subject`, `text`, and `html` are template strings. Either text or HTML can supply the message body. The legacy name `envelope_sender` is misleading: this field overrides the message's visible sender; the sending profile supplies the transport sender. See [sending profiles](../user-guide/sending-profiles.md).

An attachment contains `name`, `type` (media type), and `content` (base64). `attachments` may be `null` in a legacy response. File types have different rendering and size rules; see [attachments](../user-guide/attachments.md).

Template syntax is not limited to variable substitution, and HTML is not automatically escaped. Read the [template reference](../user-guide/template-reference.md) before placing untrusted text into HTML. Use [imports and previews](imports-and-previews.md) to inspect MIME content or render a preview before saving.
