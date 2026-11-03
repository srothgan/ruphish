---
title: Replace the IMAP read
description: Migrate GET /api/imap/ to the current Ruphish contract.
sidebar:
  order: 230
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Replace the implicit per-user IMAP read with a shared mailbox list, then retain the selected mailbox ID.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/imap/` | `GET /api/reporting-mailboxes/` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.IMAP` | `Vec<integrations::Mailbox>` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

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

## Gophish request

`GET /api/imap/`

No request body.

## Gophish response

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
  "last_login": "0001-01-01T00:00:00Z",
  "modified_date": "2026-11-03T12:00:00Z",
  "imap_freq": "300"
}
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/reporting-mailboxes/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

```json
[
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
]
```

## Behavior and verification

The old `/api/imap/` route is not retained. The replacement is an array, not a single object; zero, one, or multiple mailboxes are valid. Reads require `view_objects` and remain available when the feature is disabled. Imported names and IDs come from conversion; do not select an arbitrary first item.

Verify selection with two mailboxes and an empty list. Use `GET /api/reporting-mailboxes/{id}` for the chosen configuration and operational history.

Current contract: [Replace the IMAP read](../../../api/reporting-mailboxes.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/imap.go).
