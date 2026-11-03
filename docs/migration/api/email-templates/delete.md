---
title: Delete an email template
description: Migrate DELETE /api/templates/{id} to the current Ruphish contract.
sidebar:
  order: 124
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Read the current revision before deleting the email template. A blind Gophish write no longer succeeds.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `DELETE /api/templates/{id}` | `DELETE /api/templates/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<()>` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `If-Match` header | Absent. | Required current quoted revision; missing → 428, stale → 412. |
| Response | `success` boolean, `message` string, `data` null. | Same envelope shape. Do not compare diagnostic text. |
| Access | Resource selected within the current user. | Shared object, requiring `modify_objects`. |
| Unfinished campaign references | Resource deletion could leave campaign resources missing. | Deletion is blocked with 409 RESOURCE_IN_USE while an unfinished campaign references this resource. |

## Gophish request

`DELETE /api/templates/{id}`

No request body.

## Gophish response

```json
{
  "success": true,
  "message": "Template deleted successfully!",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request DELETE "$RUPHISH_URL/api/templates/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

## Ruphish response

```json
{
  "success": true,
  "message": "Template deleted successfully!",
  "data": null
}
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names. A stale revision must trigger a reload and review, not an unconditional retry. Deletion does not rewrite saved campaign content or recipient snapshots. Resolve unfinished campaign references before deletion; check future drafts as well.

In an isolated test, read the object, perform a conditional write, then verify that a stale revision is handled without overwriting newer work.

Current contract: [Delete an email template](../../../api/templates.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/template.go).
