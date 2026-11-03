---
title: Replace IMAP validation
description: Migrate POST /api/imap/validate to the current Ruphish contract.
sidebar:
  order: 232
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Save the mailbox configuration first, then validate that saved resource without a request body.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/imap/validate` | `POST /api/reporting-mailboxes/{id}/validate` |
| Success | `201 Created` | `200 OK` |
| Response type | `models.Response` | `integrations::Mailbox` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Input | Unsaved IMAP settings in JSON, including credentials. | Saved mailbox ID in the URL; no body. |
| Success | 201 envelope with `Successful login.`. | 200 current mailbox resource without password. |
| Connection failure | Could return 200 with `success:false`. | Failure uses a non-success HTTP status and structured error code. |
| Scope and permissions | Current user's proposed connection. | Shared saved configuration; requires `modify_objects` and enabled feature. |
| TLS and field types | Legacy IMAP fields. | Translate and save using the mailbox contract; false TLS means STARTTLS. |

## Gophish request

`POST /api/imap/validate`

```json
{
  "enabled": true,
  "host": "imap.example.com",
  "port": "993",
  "username": "reports@example.com",
  "password": "example-password",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported_campaign_email": false,
  "imap_freq": "300"
}
```

## Gophish response

```json
{
  "success": true,
  "message": "Successful login.",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request POST "$RUPHISH_URL/api/reporting-mailboxes/1/validate" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

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
  "id": 1,
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 1,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

## Behavior and verification

Validation opens a real mailbox connection and checks the saved credentials. It does not schedule a report scan. Use the separate check action when report processing is intended. Imported mailbox processing boundaries and closed-campaign rules are explained in the data-conversion guide.

Handle an unsuccessful connection as a failed HTTP operation, and avoid displaying a success message solely because JSON could be decoded.

Current contract: [Replace IMAP validation](../../../api/reporting-mailboxes.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/imap.go).
