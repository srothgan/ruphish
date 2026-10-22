---
title: Reporting mailboxes API
description: Configure IMAP reporting sources and inspect validation, polling, and resume state.
sidebar:
  order: 16
---

Reporting mailboxes match reported messages to campaign recipients. Multiple mailbox resources replace the single-setting model used by older integrations.

| Method | Path | Purpose |
| --- | --- | --- |
| GET, POST | `/api/reporting-mailboxes/` | List or create owned mailboxes |
| GET, PUT, DELETE | `/api/reporting-mailboxes/{id}` | Read, edit, or delete configuration |
| POST | `/api/reporting-mailboxes/{id}/validate` | Test configuration; return updated resource |
| POST | `/api/reporting-mailboxes/{id}/check` | Request a poll; returns `202` with resource state |
| POST | `/api/reporting-mailboxes/{id}/resume` | Resume after a mailbox-identity change |

## Configuration

Writes contain `name`, `enabled`, `host`, `port`, `username`, optional `password`, `tls`, `folder`, `restrict_domain`, `delete_reported`, `frequency`, and expected `revision` for edits. Unknown fields are rejected. Poll frequency is in seconds, from 30 through 31,540,000.

`tls: true` selects implicit TLS; false selects STARTTLS, not unencrypted authentication. Username/password authentication is supported. OAuth is not part of this interface. Omitting a password on an edit retains the saved secret; responses expose `has_password`, not the password.

Responses add `id`, `revision`, `last_check`, `last_success`, `next_check`, `running`, `code`, and recent `history`. Run timestamps use UTC Unix seconds. History includes fixed outcome codes and matched/rejected counts; a successful connection test does not mean a report was matched.

Send the expected revision on edits and use `If-Match` when deleting to protect against concurrent changes. Resume requires a JSON body containing `revision`. It deliberately establishes a new-arrival checkpoint after UIDVALIDITY changes; it is not a request to process all historical mail.

## Matching and acknowledgement

The first successful poll checkpoints existing messages. New forwarded content must contain an unambiguous matching campaign origin and recipient identifier owned by the mailbox's account. A sender-domain filter narrows candidates but is not sender authentication.

Matched outcomes commit before mailbox acknowledgement. Retrying acknowledgement must not create another report event. Deletion, when enabled, requires UID-scoped behavior rather than a broad mailbox expunge. Read [reporting mailboxes](../user-guide/reporting-mailboxes.md) before enabling it.

Legacy `/api/imap/` GET/POST and `/api/imap/validate` remain compatibility surfaces. New integrations should use the resource routes above; a legacy validation GET is rejected instead of initiating an external check.

GET `/api/imap/` returns an array containing the caller's lowest-ID mailbox, or `[]`. Its compatibility shape uses string `port` and `imap_freq`, `delete_reported_campaign_email` instead of `delete_reported`, and `last_login`/`modified_date`. `password` is always empty and `has_password` indicates whether a secret exists; `ignore_cert_errors` is always false.

POST `/api/imap/` accepts `enabled`, `host`, string `port`, `username`, optional `password`, `tls`, `ignore_cert_errors`, `folder`, `restrict_domain`, `delete_reported_campaign_email`, and string `imap_freq`. It updates that lowest-ID mailbox or creates one. Empty folder becomes `INBOX`; invalid/out-of-range frequency falls back to 60 seconds. An empty password preserves the existing one. `ignore_cert_errors: true` is rejected. Successful save returns `201` with a success envelope.

POST `/api/imap/validate` takes the same compatibility settings without saving them. Successful validation returns `201`; an integration failure can return `200` with `success: false`, `code`, and `message`. This legacy exception means HTTP success alone is insufficient: inspect the envelope. Prefer the resource-specific validation route for new code.

An email-client reporting button can instead call the recipient's public [report endpoint](participant-endpoints.md). That uses a campaign tracking identity, not an administrative API key, and is separate from IMAP polling.
