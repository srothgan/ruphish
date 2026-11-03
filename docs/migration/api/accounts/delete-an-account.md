---
title: Delete an account
description: Migrate DELETE /api/users/{id} to the current Ruphish contract.
sidebar:
  order: 284
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Use an administrator to delete another account; shared campaign data remains.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `DELETE /api/users/{id}` | `DELETE /api/users/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<()>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Self-deletion | Handler allowed users to address their own account, subject to model checks. | Rejected. Use an authorized administrator to delete another account. |
| Last administrator | Protection against deleting the last admin. | Protection includes availability; do not remove the last available administrator. |
| Success body | `success`, `message`, `data`. | Same envelope shape. |
| Existing shared objects | Account-owned data could be removed with account deletion. | Creator attribution is not ownership isolation; do not use account deletion as a resource cleanup operation. |

## Gophish request

`DELETE /api/users/{id}`

No request body.

## Gophish response

```json
{
  "success": true,
  "message": "User deleted Successfully!",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request DELETE "$RUPHISH_URL/api/users/2" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

```json
{
  "success": true,
  "message": "User deleted Successfully!",
  "data": null
}
```

## Behavior and verification

Requires `modify_objects` and `modify_system` to delete another account. Shared resources and campaigns remain, and attribution retains the deleted account identity. Use explicit resource/campaign APIs for intended data removal.

With disposable accounts, verify self-deletion is rejected and deleting another account preserves shared data and its attribution.

Current contract: [Delete an account](../../../api/accounts-and-keys.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/user.go).
