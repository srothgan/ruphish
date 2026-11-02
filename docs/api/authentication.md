---
title: API authentication
description: Authenticate scripts with an API key and keep browser authentication separate.
sidebar:
  order: 2
---

An API key acts as its application account. It does not bypass that account's permissions.

## Send a bearer token

Send `Authorization: Bearer <key>` on each request. Use HTTPS and a trusted certificate. Avoid putting keys in URLs, shell history, screenshots, or logs.

For legacy clients, Ruphish also accepts the `api_key` query parameter. The key-reset operation accepts a form API key. Prefer the bearer header for new code because URLs are more likely to be recorded.

An explicitly supplied invalid key fails authentication; the server does not fall back to an existing browser cookie. Locked accounts and accounts that must finish password setup cannot use a key to bypass those restrictions.

## Permissions and shared access

Reading campaign resources requires `view_objects`; changing them requires `modify_objects`. System administration uses `modify_system`. Resources, campaigns, results and exports are shared across the installation. See the [permission matrix](../administration/accounts-and-permissions.md).

| Endpoint group | Read boundary | Write or external-action boundary |
| --- | --- | --- |
| Groups, templates, pages, SMTP, campaigns, drafts, domains | `view_objects` | `modify_objects` and state/revision checks |
| Analysis, exports, dashboard | `view_objects` | Read-only endpoints |
| Imports, previews, SMTP/campaign tests | Not ordinary resource reads | `modify_objects`; a test send has an external effect |
| Reporting mailboxes | `view_objects` | `modify_objects` and revision checks |
| Webhooks | `view_objects` | `modify_objects`; the administrator-controlled feature switch must permit saves and network actions |
| Other application accounts | System-administration boundary | System administration and applicable account-mutation rules |

Operation-specific invariants still apply after permission checks, including a retained usable administrator, revision checks, active-campaign resource dependencies, and delivery generations.

## Browser sessions

`/auth/session` supplies browser session bootstrap data. Session actions use POST `/auth/login`, `/auth/logout`, `/auth/password/reset`, `/auth/impersonate` and `/auth/impersonate/exit`. The corresponding browser pages keep `/login`, `/logout` and `/reset_password`; those page URLs accept no mutation requests. `/internal/` includes browser account operations such as password and preference changes. Session cookies and CSRF checks protect that flow; an API key is not a replacement for every browser-specific requirement.

External clients should use documented `/api/` routes. See [users and API keys](users-and-api-keys.md) for key rotation and account management.

## Read-only attribution

Resource, campaign and draft responses include nullable `created_by` and `edited_by` objects and a nullable UTC `edited_at` timestamp. Each account reference contains only `{ "id": 2, "username": "operator", "deleted": false }`; it never contains credentials, permissions or preferences. A resource response includes everything needed to display attribution, without a separate account lookup. List responses use a batched server-side lookup.

The server derives attribution from authenticated writes; clients cannot assign it. Imported records retain creator IDs where known and leave unverified last-editor fields null. A known historical ID with no retained identity has `username: null` and `deleted: false`; the UI displays its ID rather than attributing it to someone else. Deleted accounts retain their display name with `deleted: true`. Frozen campaign resources retain their launch-time creator/editor IDs and edit timestamp; display names reflect the current account name or the name retained at deletion.
