---
title: Users and API keys
description: Manage application accounts and rotate the caller's API key.
sidebar:
  order: 15
---

These users are application accounts. Campaign recipients belong to [groups](groups.md) and have no administrative login.

| Method | Path | Purpose |
| --- | --- | --- |
| GET, POST | `/api/users/` | List accounts or create one, subject to permission |
| GET, PUT, DELETE | `/api/users/{id}` | Read, replace editable account settings, or delete |
| POST | `/api/reset` | Rotate the authenticated caller's API key |

## Account fields

Writes use `username`, `password`, `role` (role slug), `password_change_required`, and `account_locked`. The response includes `id`, `username`, `api_key`, a `role` object with slug/name/description, the two account flags, and `last_login`. Treat this response as credential-bearing data; do not put it in ordinary application logs.

An empty password on an eligible edit preserves the existing password. Password changes have different authorization rules from profile edits: use the browser's current-password flow for self-service password changes. Generic self-update is not an API shortcut around current-password verification.

The server protects the last unlocked administrator and rejects self-deletion. System administration permission does not remove these invariants. Read [accounts and permissions](../administration/accounts-and-permissions.md) before automating account provisioning.

## Rotate your key

POST `/api/reset` accepts form input and can authenticate through the bearer header. A successful response has `success: true`, a message, and the new key in `data`. Replace the key in your secret store before the next authenticated request; the old key stops working.

If the rotation response is lost, do not assume the old key is still valid. Recover through the authenticated account interface or your established administrator recovery process. Ruphish does not provide an unauthenticated key-recovery endpoint.

Display language, date/time locale, and time-zone preferences belong to the browser account flow under `/internal/preferences`. They are deliberately separate from this public account-management contract. See [account preferences](../user-guide/account.md).
