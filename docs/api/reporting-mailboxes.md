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

Writes contain `name`, `enabled`, `host`, `port`, `username`, optional `password`, `tls`, optional `ignore_cert_errors`, `folder`, `restrict_domain`, `delete_reported`, `frequency`, and expected `revision` for edits. Unknown fields are rejected. Poll frequency is in seconds, from 30 through 31,540,000.

`tls: true` selects implicit TLS; false selects STARTTLS, not unencrypted authentication. Username/password authentication is supported. OAuth is not part of this interface. Omitting a password on an edit retains the saved secret; responses expose `has_password`, not the password.

`ignore_cert_errors` defaults to false. Explicitly enabling it skips server certificate chain, validity and hostname checks while retaining encryption and TLS handshake signature verification. The same policy applies to validation and background polling; there is no automatic bypass or plaintext fallback. Editing the policy increments the configuration revision and preserves UID checkpoints and pending acknowledgements. Workers recheck the revision before sending credentials and committing results; authentication already authorized when an edit commits may be in flight.

Responses add `id`, `revision`, `last_check`, `last_success`, `next_check`, `running`, `code`, and recent `history`. Run timestamps use UTC Unix seconds. History includes fixed outcome codes and matched/rejected counts; a successful connection test does not mean a report was matched.

Send the expected revision on edits and use `If-Match` when deleting to protect against concurrent changes. Resume requires a JSON body containing `revision`. It deliberately establishes a new-arrival checkpoint after UIDVALIDITY changes; it is not a request to process all historical mail.

## Matching and acknowledgement

At creation, the initial scan boundary is the earliest actual email-send observation across all non-completed campaigns, independently of their creators. The first successful poll translates it into a durable UID checkpoint; without an eligible send, it checkpoints existing messages and starts with new arrivals. The boundary is internal, not a configurable API field. Ordinary edits, restarts, and campaign lifecycle changes preserve progress. See the [initial discovery and timezone behavior](../user-guide/reporting-mailboxes.md#send-a-report).

Forwarded content must contain an unambiguous matching campaign origin and recipient identifier in this installation. The mailbox creator does not restrict eligible campaign owners. A sender-domain filter narrows candidates but is not sender authentication. Reports for completed campaigns produce the bounded operational reason `mailbox_campaign_closed` and advance the checkpoint without changing campaign results or queuing webhooks.

Matched outcomes commit before mailbox acknowledgement. Retrying acknowledgement must not create another report event, including when the campaign has since completed. Deletion, when enabled, requires UID-scoped behavior rather than a broad mailbox expunge. Read [reporting mailboxes](../user-guide/reporting-mailboxes.md) before enabling it.

The singleton `/api/imap/` and `/api/imap/validate` routes have been removed. Use the reporting-mailbox resources above with numeric `port` and `frequency`, explicit mailbox IDs and revision-checked edits. Create or update a mailbox before validating its saved configuration.

An email-client reporting button can instead call the recipient's public [report endpoint](participant-endpoints.md). That uses a campaign tracking identity, not an administrative API key, and is separate from IMAP polling.
