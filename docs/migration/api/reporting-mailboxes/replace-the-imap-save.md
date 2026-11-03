---
title: Replace the IMAP save
description: Migrate POST /api/imap/ to the current Ruphish contract.
sidebar:
  order: 231
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update a selected mailbox by ID and body revision instead of overwriting the current user's IMAP settings.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/imap/` | `PUT /api/reporting-mailboxes/{id}` |
| Success | `201 Created` | `200 OK` |
| Response type | `models.Response` | `integrations::Mailbox` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Resource identity | One IMAP configuration selected implicitly by the current user. | Named shared mailbox resources with integer `id`; list, select, then update the chosen ID. |
| `port` | JSON string, for example `"993"`. | JSON integer, for example `993`; range 1–65535. |
| `imap_freq` | JSON string interval in seconds. | Renamed to `frequency`, an integer from 30 to 31540000. |
| `delete_reported_campaign_email` | Boolean. | Renamed to `delete_reported`, still boolean. |
| `name` | Absent. | Required string for create/update. |
| `password` | String included in read response. | Write-only. Omitted/null preserves on update; nonempty required for creation. Response uses boolean `has_password`. |
| `tls` | Boolean: true selects implicit TLS; false uses a plaintext connection. | Boolean: true uses implicit TLS; false requires STARTTLS. It is not permission for plaintext login. |
| `enabled`, `host`, `username`, `folder`, `restrict_domain`, `ignore_cert_errors` | Boolean/string connection fields. | Names/types retained; current connection and validation rules apply. |
| `last_login`, `modified_date` | RFC 3339 strings in legacy configuration. | Use attribution timestamps and operational `last_check`, `last_success`, `next_check` Unix-second integers; these are different observations, not simple renames. |
| `revision`, attribution, `history`, `running`, `code` | Absent. | Added concurrency, attribution, operational history, boolean running state, and diagnostic code. |
| Update precondition | No explicit version. | JSON integer `revision` from the last read; missing → 428, stale → 412. |
| Success | 201 envelope confirming settings saved. | 200 mailbox object including new revision; no returned password. |

## Gophish request

`POST /api/imap/`

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
  "message": "Successfully saved IMAP settings.",
  "data": null
}
```

## Ruphish request

Request type: `integrations::request::MailboxWrite`. Save this body as `request.json`:

```json
{
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
  "revision": 1
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/reporting-mailboxes/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
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
  "revision": 2,
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

Existing imported mailboxes should be edited, not recreated on every save. For an additional mailbox use `POST /api/reporting-mailboxes/`, omit revision, and provide a password. Writes require `modify_objects` and the global feature to be enabled. Unknown write fields are rejected; translate names/types and remove old read-only fields.

Select an imported mailbox, update without sending its password, and verify `has_password` remains true. Check a stale revision and a disabled-feature response. Creation and the new check/resume/delete actions are documented in the current reference.

Current contract: [Replace the IMAP save](../../../api/reporting-mailboxes.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/imap.go).
