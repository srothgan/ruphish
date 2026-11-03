---
title: Overview
description: Dashboard inventory, attention items, and acceptance-cohort posture.
sidebar:
  order: 25
---

Both overview endpoints require `view_objects`. They return aggregate dashboard projections without full resource content or recipient identities.

## GET `/api/overview`

Read recent campaigns, inventory counts, and operational attention items.

**Request:** No body. **Response:** `200 OK`, `overview::Snapshot` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/overview" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "as_of": 1793707200,
  "recent": [
    {
      "id": 1,
      "name": "Training campaign",
      "status": "Queued",
      "launch": "2030-01-01T00:00:00Z",
      "recipients": 1,
      "outcomes": {
        "delivered": 0,
        "opened": 0,
        "clicked": 0,
        "reported": 0,
        "credentials_entered": 0
      }
    }
  ],
  "inventory": {
    "templates": 1,
    "pages": 1,
    "groups": 1,
    "profiles": 1,
    "domains": 1,
    "domains_verified": 0,
    "reporting_mailboxes": 0
  },
  "attention": {
    "unverified_enabled_domains": 0,
    "delivery_problems": 0,
    "campaign_ids": [],
    "next_launch": {
      "id": 1,
      "name": "Training campaign",
      "launch": "2030-01-01T00:00:00Z"
    }
  }
}
```

### Snapshot fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `as_of` | `integer` | Server time used consistently for expiry and launch calculations. |
| `recent` | `RecentCampaign[]` | Most recent campaigns, newest first. |
| `inventory` | `Inventory` | Resource inventory. |
| `attention` | `Attention` | Attention items. |

### RecentCampaign fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Campaign identity. |
| `name` | `string` | Display name. |
| `status` | `CampaignStatus` | Lifecycle state. |
| `launch` | `string or null` | Normalized UTC launch, or explicitly unknown. |
| `recipients` | `integer` | Audience size. |
| `outcomes` | `Posture` | SMTP-accepted recipients and cumulative progression for this campaign. |

### Inventory fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `templates` | `integer` | Email templates. |
| `pages` | `integer` | Landing pages. |
| `groups` | `integer` | Recipient groups. |
| `profiles` | `integer` | Sending profiles. |
| `domains` | `integer` | Configured domains. |
| `domains_verified` | `integer` | Domains currently verified. |
| `reporting_mailboxes` | `integer` | Reporting mailbox resources. |

### Attention fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `unverified_enabled_domains` | `integer` | Enabled domains that are not currently verified. |
| `delivery_problems` | `integer` | Current failed, blocked or uncertain deliveries in unfinished campaigns. |
| `campaign_ids` | `integer[]` | Up to ten unfinished campaign identities requiring delivery review. |
| `next_launch` | `NextLaunch or null` | The soonest future scheduled launch, if any. |

### NextLaunch fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Campaign identity. |
| `name` | `string` | Display name. |
| `launch` | `string` | Scheduled launch as stored (ISO-8601). |

## GET `/api/overview/posture`

Read recipient engagement for an SMTP-acceptance cohort.

**Request:** Query: overview::request::WindowQuery. **Response:** `200 OK`, `overview::PostureResponse` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/overview/posture?window=all" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "window": "all",
  "since": null,
  "as_of": 1793707200,
  "delivered": 0,
  "opened": 0,
  "clicked": 0,
  "reported": 0,
  "credentials_entered": 0,
  "previous": null
}
```

`window` accepts `all` (default), `7d`, `30d`, or `365d`. Window membership is based on first recorded SMTP acceptance; current outcomes are observed through `as_of`. Repeated events count once per campaign recipient.

### Posture response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `window` | `string` | all, 7d, 30d or 365d. |
| `since` | `integer or null` | Inclusive acceptance-time lower bound; absent for all time. |
| `as_of` | `integer` | Inclusive server observation time in Unix seconds. |
| `previous` | `PreviousPeriod or null` | Preceding equal-length period, absent for all time. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `delivered` | `integer` | Recipients a message was recorded as sent to. |
| `opened` | `integer` | Recipients with an observed open, click or submission. |
| `clicked` | `integer` | Recipients with an observed click or submission. |
| `reported` | `integer` | Recipients who reported a message. |
| `credentials_entered` | `integer` | Recipients whose submission explicitly recorded password presence. |

The count fields are flattened into the response, as shown above. Submissions contribute to clicked and opened; clicks contribute to opened. `credentials_entered` requires explicit positive password-presence evidence.

### Previous period

| Attribute | Type | Contract |
| --- | --- | --- |
| `since` | `integer` | Inclusive acceptance-time lower bound. |
| `until` | `integer` | Exclusive upper bound for acceptance and observations. |
| `counts` | `InteractionCounts` | Interaction counts only; send-volume comparisons are not exposed. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `opened` | `integer` | Recipients with a recorded open. |
| `clicked` | `integer` | Recipients with a recorded click. |
| `reported` | `integer` | Recipients with a recorded report. |
| `credentials_entered` | `integer` | Recipients with explicitly recorded password presence. |

`previous` is null for all time. Other windows include the immediately preceding equal-length acceptance period; its `until` boundary is exclusive. Previous counts do not include a delivered metric.
