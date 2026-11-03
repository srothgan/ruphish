---
title: Delete a campaign
description: Migrate DELETE /api/campaigns/{id} to the current Ruphish contract.
sidebar:
  order: 207
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Keep DELETE, but account for shared access and the removal of campaign history.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `DELETE /api/campaigns/{id}` | `DELETE /api/campaigns/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<()>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Method | `DELETE`. | `DELETE`; requires `modify_objects`. |
| Response | Envelope with boolean `success`, string `message`, nullable `data`. | Same shape. Branch on HTTP status/code for failures, not message wording. |
| Access | Campaign belonging to current user. | Shared campaign. |

## Gophish request

`DELETE /api/campaigns/{id}`

No request body.

## Gophish response

```json
{
  "success": true,
  "message": "Campaign deleted successfully!",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request DELETE "$RUPHISH_URL/api/campaigns/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

```json
{
  "success": true,
  "message": "Campaign deleted successfully!",
  "data": null
}
```

## Behavior and verification

Deletion removes the campaign and its stored results/history. It cannot retract messages or webhook deliveries. Export any history you need before deletion.

Use a disposable campaign to verify the method and resulting state; do not test destructive operations against retained campaign history.

Current contract: [Delete a campaign](../../../api/campaigns.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/campaign.go).
