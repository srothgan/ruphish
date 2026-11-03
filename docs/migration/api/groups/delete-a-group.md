---
title: Delete a group
description: Migrate DELETE /api/groups/{id} to the current Ruphish contract.
sidebar:
  order: 106
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Read the current revision before deleting the group. A blind Gophish write no longer succeeds.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `DELETE /api/groups/{id}` | `DELETE /api/groups/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<()>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `If-Match` header | Absent. | Required current quoted revision; missing → 428, stale → 412. |
| Response | `success` boolean, `message` string, `data` null. | Same envelope shape. Do not compare diagnostic text. |
| Access | Resource selected within the current user. | Shared object, requiring `modify_objects`. |

## Gophish request

`DELETE /api/groups/{id}`

No request body.

## Gophish response

```json
{
  "success": true,
  "message": "Group deleted successfully!",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request DELETE "$RUPHISH_URL/api/groups/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

## Ruphish response

```json
{
  "success": true,
  "message": "Group deleted successfully!",
  "data": null
}
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names. A stale revision must trigger a reload and review, not an unconditional retry. Deletion does not rewrite saved campaign content or recipient snapshots. Check saved drafts before deleting a source group.

In an isolated test, read the object, perform a conditional write, then verify that a stale revision is handled without overwriting newer work.

Current contract: [Delete a group](../../../api/groups.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/group.go).
