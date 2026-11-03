---
title: Complete a campaign
description: Migrate GET /api/campaigns/{id}/complete to the current Ruphish contract.
sidebar:
  order: 206
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Change completion to POST; GET must not be used for this mutation.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/campaigns/{id}/complete` | `POST /api/campaigns/{id}/complete` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<()>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Method | `GET`. | `POST`; requires `modify_objects`. |
| Response | Envelope with boolean `success`, string `message`, nullable `data`. | Same shape. Branch on HTTP status/code for failures, not message wording. |
| Access | Campaign belonging to current user. | Shared campaign. |

## Gophish request

`GET /api/campaigns/{id}/complete`

No request body.

## Gophish response

```json
{
  "success": true,
  "message": "Campaign completed successfully!",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/1/complete" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

```json
{
  "success": true,
  "message": "Campaign completed successfully!",
  "data": null
}
```

## Behavior and verification

Completion closes participant interactions and cancels pending delivery work. A send already in flight can finish; completion cannot retract SMTP-accepted mail.

Use a disposable campaign to verify the method and resulting state; do not test destructive operations against retained campaign history.

Current contract: [Complete a campaign](../../../api/campaigns.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/campaign.go).
