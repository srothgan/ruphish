---
title: Sending profiles API
description: Configure SMTP transport and test it without confusing acceptance with delivery.
sidebar:
  order: 8
---

| Method | Path | Result |
| --- | --- | --- |
| GET, POST | `/api/smtp/` | List or create sending profiles |
| GET, PUT, DELETE | `/api/smtp/{id}` | Read, replace, or delete a profile |
| GET | `/api/smtp/summary` | `total` and resource `items` |

## Profile fields

| Field | Meaning |
| --- | --- |
| `name` | Resource name |
| `interface_type` | Use `SMTP` |
| `host` | SMTP host, optionally with a port |
| `from_address` | Sender mailbox, optionally with a display name |
| `username`, `password` | Optional SMTP credentials; treat returned profile data as sensitive |
| `ignore_cert_errors` | Whether to bypass SMTP certificate verification; keep false for normal use |
| `headers` | Array of objects containing `key` and `value`; `[]` when empty |

The response adds `id` and `modified_date`. PUT requires a matching `id`; use `If-Match` to prevent overwriting a concurrent edit.

This example targets only the canonical local fixture's mail-capture container:

```json
{
  "name": "Local inbox",
  "interface_type": "SMTP",
  "host": "172.30.53.20:1025",
  "from_address": "Training <training@example.test>",
  "ignore_cert_errors": false,
  "headers": []
}
```

It is not a public mail server. Use it from the [local tutorial](../getting-started/first-campaign.md), where the application and inbox share the fixture network.

Port 465 uses implicit TLS; other ports negotiate STARTTLS when offered. Authentication is restricted on unencrypted connections. For exact operational behavior and trust choices, read [sending profiles](../user-guide/sending-profiles.md).

`POST /api/util/test_connection` tests a profile-shaped SMTP configuration. `POST /api/util/send_test_email` sends a message with template, page, profile, URL, and recipient inputs; see [imports and previews](imports-and-previews.md). A successful SMTP submission means the server accepted responsibility, not that a person received or read the message.
