---
title: API authentication
description: Authenticate scripts with an API key and keep browser authentication separate.
sidebar:
  order: 2
---

An API key acts as its application account. It does not bypass that account's permissions or ownership boundaries.

## Send a bearer token

Send `Authorization: Bearer <key>` on each request. Use HTTPS and a trusted certificate. Avoid putting keys in URLs, shell history, screenshots, or logs.

For legacy clients, Ruphish also accepts the `api_key` query parameter. The key-reset operation accepts a form API key. Prefer the bearer header for new code because URLs are more likely to be recorded.

An explicitly supplied invalid key fails authentication; the server does not fall back to an existing browser cookie. Locked accounts and accounts that must finish password setup cannot use a key to bypass those restrictions.

## Permissions and ownership

Reading campaign resources requires `view_objects`; changing them requires `modify_objects`. System administration uses `modify_system`. Resources remain scoped to their owner: being an administrator does not turn a campaign list into an installation-wide data export. See the [permission matrix](../administration/accounts-and-permissions.md).

| Endpoint group | Read boundary | Write or external-action boundary |
| --- | --- | --- |
| Groups, templates, pages, SMTP, campaigns, drafts, domains | `view_objects` and owner access | `modify_objects` and owner/state checks |
| Analysis, exports, dashboard | `view_objects` and owner access | Read-only endpoints |
| Imports, previews, SMTP/campaign tests | Not ordinary resource reads | `modify_objects`; a test send has an external effect |
| Reporting mailboxes | `view_objects` and owner access | `modify_objects` and owner/revision checks |
| Webhooks | Resource-read permission plus `modify_system`; installation-wide configuration | Resource-write permission plus `modify_system` |
| Other application accounts | System-administration boundary | System administration and applicable account-mutation rules |

Operation-specific invariants still apply after permission checks, including a retained usable administrator, revision checks, active-campaign resource dependencies, and delivery generations.

## Browser sessions

`/auth/session` supplies browser session bootstrap data. Legacy browser form paths such as `/login`, `/logout`, and `/reset_password` also remain. `/internal/` includes browser account operations such as password and preference changes. Session cookies and CSRF checks protect that flow; an API key is not a replacement for every browser-specific requirement.

External clients should use documented `/api/` routes. See [users and API keys](users-and-api-keys.md) for key rotation and account management.
