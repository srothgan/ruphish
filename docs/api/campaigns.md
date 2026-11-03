---
title: Campaigns
description: Preparation, creation, execution reads, duplication, test sends, and completion.
sidebar:
  order: 20
---

Campaigns are executions. Saving incomplete configuration uses [campaign drafts](campaign-drafts.md). Reads require `view_objects`; preparation, test sends, creation, completion, and deletion require `modify_objects`. Campaign request bodies are limited to 1 MiB.

## Prepare and launch

`campaign::Create` is accepted by preparation and creation, and nested inside test and draft-launch requests. Unknown fields are rejected.

| Attribute | Type | Contract |
| --- | --- | --- |
| `learning_revision` | `string or null, optional` | Reviewed learning content revision, when a learning page is selected. |
| `name` | `string` | Campaign display name. |
| `url` | `string` | Public URL template. |
| `selection` | `Selections` | Shared resource IDs and optional reviewed revisions. |
| `launch_date` | `string or null` | UTC or offset launch; exclusive with launch_local. Both absent means now. |
| `send_by_date` | `string or null` | UTC or offset distribution end; exclusive with send_by_local. |
| `policy` | `Policy or null` | Optional open tracking and destination choices. |
| `launch_local` | `LocalTime or null` | Named-zone launch input; exclusive with launch_date. |
| `send_by_local` | `LocalTime or null` | Named-zone distribution end; exclusive with send_by_date. |

### Selection

| Attribute | Type | Contract |
| --- | --- | --- |
| groups | ReviewedResource[] | Ordered recipient groups; at least one is required. |
| template | ReviewedResource | Saved email template. |
| page | ReviewedResource | Saved landing page. |
| smtp | ReviewedResource | Saved sending profile. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Shared persistent resource ID. |
| `revision` | `string or null` | Revision shown during review, if already reviewed. |

Preparation resolves missing revisions. For creation, send the reviewed revisions as strings; they detect changes between review and execution. An integer revision used elsewhere in the API is not interchangeable with this string field.

### Policy and schedule

| Attribute | Type | Contract |
| --- | --- | --- |
| `track_opens` | `boolean` | Whether email-open observations are enabled for this campaign. |
| `redirect_url` | `string or null` | Post-submission redirect override. |
| `learning_page_id` | `integer or null, optional` | Selected shared learning page; custom redirect must be empty when present. |

Absent policy defaults to open tracking enabled and the page redirect default. A null `redirect_url` inherits the page value; an empty string explicitly disables the custom redirect. A learning page and a nonempty custom redirect are mutually exclusive. When selecting learning content, carry its reviewed `learning_revision` into the launch command.

Absolute schedule fields accept RFC 3339 strings with an offset. Local schedule objects have `local` (for example `2030-01-01T09:00`) and `time_zone` (for example `Europe/Berlin`). Supply either absolute or local input for each boundary. Missing launch input means immediate launch; missing send-by input means no distribution deadline. Local times are resolved on the server; invalid or ambiguous times are rejected.

## POST `/api/campaigns/prepare`

Resolve selections, deduplicate the audience, validate the destination, and render a sample without creating an execution.

**Request:** campaign::Create. **Response:** `200 OK`, `campaign_workflow::Review` as `application/json`.

Request body (`request.json`):

```json
{
  "name": "Training campaign",
  "url": "http://127.0.0.1:8081",
  "selection": {
    "groups": [
      {
        "id": 1,
        "revision": "1"
      }
    ],
    "template": {
      "id": 1,
      "revision": "1"
    },
    "page": {
      "id": 1,
      "revision": "1"
    },
    "smtp": {
      "id": 1,
      "revision": "1"
    }
  },
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  },
  "launch_local": null,
  "send_by_local": null
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/prepare" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "learning_revision": null,
  "warnings": [],
  "total": 1,
  "duplicate_count": 0,
  "launch_date": 1893456000,
  "send_by_date": null,
  "selection": {
    "groups": [
      {
        "id": 1,
        "revision": "1",
        "name": "Training recipients"
      }
    ],
    "template": {
      "id": 1,
      "revision": "1",
      "name": "Training message"
    },
    "page": {
      "id": 1,
      "revision": "1",
      "name": "Training page"
    },
    "smtp": {
      "id": 1,
      "revision": "1",
      "name": "Training sender"
    }
  },
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  },
  "mailer_enabled": true,
  "conflict_count": 0,
  "conflicts": [],
  "preview": {
    "subject": "Training notification",
    "html": "<p>Hello Ada</p>",
    "text": "Hello Ada",
    "visible_sender": "training@example.com",
    "transport_sender": "training@example.com"
  }
}
```

### Review fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `learning_revision` | `string or null` | Learning-page revision included in the reviewed launch. |
| `warnings` | `resource_details::PageWarning[]` | Advisory findings on the processed landing page. |
| `total` | `integer` | Number of unique recipients. |
| `duplicate_count` | `integer` | Number of discarded duplicate entries. |
| `launch_date` | `integer` | Resolved launch time in UTC seconds. |
| `send_by_date` | `integer or null` | Optional distribution end in UTC seconds. |
| `selection` | `Selection<ReviewedResource>` | Resolved resource selections. |
| `policy` | `Policy` | Effective tracking policy confirmed by review. |
| `mailer_enabled` | `boolean` | Whether delivery is enabled by startup configuration. |
| `conflict_count` | `integer` | Total conflicts including those beyond the displayed sample. |
| `conflicts` | `Conflict[]` | Bounded conflicting recipient sample. |
| `preview` | `Preview` | Rendered sample using the first recipient. |

The audience is deduplicated by email, retaining the first occurrence in group and recipient order. `duplicate_count` counts discarded rows; differing recipient attributes are reported as conflicts without creating multiple deliveries. `selection` adds `name` to each reviewed resource.

| Attribute | Type | Contract |
| --- | --- | --- |
| `email` | `string` | Recipient mailbox identity. |
| `kept` | `RecipientAttributes` | Attributes retained from the first occurrence. |
| `discarded` | `RecipientAttributes` | Conflicting attributes from a later occurrence. |

`kept` and `discarded` each contain `first_name`, `last_name`, and `position` strings. The conflict sample is bounded; use `conflict_count` for the full count.

| Attribute | Type | Contract |
| --- | --- | --- |
| `subject` | `string` | Rendered subject. |
| `html` | `string` | Rendered HTML body. |
| `text` | `string` | Rendered plain-text body. |
| `visible_sender` | `string` | Rendered visible From address. |
| `transport_sender` | `string` | SMTP envelope sender. |

## POST `/api/campaigns/`

Create an execution using the reviewed resource selections.

**Request:** campaign::Create. **Response:** `201 Created`, `campaign_details::Campaign` as `application/json`.

Use a fresh key for each intended execution. Reuse the same key and typed request after an uncertain response; a different request with the same key conflicts. Keys are nonempty and at most 128 bytes. The header is optional for direct creation, but omitting it removes this retry protection.

Request body (`request.json`):

```json
{
  "name": "Training campaign",
  "url": "http://127.0.0.1:8081",
  "selection": {
    "groups": [
      {
        "id": 1,
        "revision": "1"
      }
    ],
    "template": {
      "id": 1,
      "revision": "1"
    },
    "page": {
      "id": 1,
      "revision": "1"
    },
    "smtp": {
      "id": 1,
      "revision": "1"
    }
  },
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  },
  "launch_local": null,
  "send_by_local": null
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'Idempotency-Key: training-campaign-2030-01' \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
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
  "track_opens": false,
  "id": 1,
  "name": "Training campaign",
  "status": "Queued",
  "created_date": "2026-11-03T12:00:00Z",
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "completed_date": null,
  "url": "http://127.0.0.1:8081",
  "template": {
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
    "revision": 1,
    "id": 1,
    "name": "Training message",
    "modified_date": "2026-11-03T12:00:00Z",
    "subject": "Training notification",
    "text": "Hello {{.FirstName}}",
    "html": "<p>Hello {{.FirstName}}</p>",
    "envelope_sender": "",
    "attachments": []
  },
  "page": {
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
    "revision": 1,
    "id": 1,
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<h1>Training page</h1>",
    "redirect_url": ""
  },
  "smtp": {
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
    "revision": 1,
    "id": 1,
    "name": "Training sender",
    "modified_date": "2026-11-03T12:00:00Z",
    "interface_type": "SMTP",
    "host": "smtp.example.com:587",
    "from_address": "training@example.com",
    "ignore_cert_errors": false,
    "headers": [],
    "username": "training@example.com",
    "has_password": true
  },
  "results": [
    {
      "id": "AbCd1234",
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer",
      "status": "Scheduled",
      "ip": "",
      "latitude": 0.0,
      "longitude": 0.0,
      "reported": false,
      "send_date": "2030-01-01T00:00:00Z",
      "modified_date": "2026-11-03T12:00:00Z"
    }
  ],
  "groups": [
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
      "revision": 1,
      "id": 1,
      "name": "Training recipients",
      "modified_date": "2026-11-03T12:00:00Z",
      "targets": [
        {
          "email": "ada@example.com",
          "first_name": "Ada",
          "last_name": "Lovelace",
          "position": "Engineer"
        }
      ]
    }
  ]
}
```

### Campaign response

| Attribute | Type | Contract |
| --- | --- | --- |
| `track_opens` | `boolean` | Whether this campaign collects email-open observations. |
| `id` | `integer` | Execution identity. |
| `name` | `string` | Display name. |
| `status` | `CampaignStatus` | Campaign lifecycle. |
| `created_date` | `string or null` | Creation timestamp, or explicitly unknown. |
| `launch_date` | `string or null` | Launch timestamp, or explicitly unknown. |
| `send_by_date` | `string or null` | Distribution end, or no deadline. |
| `completed_date` | `string or null` | Completion timestamp, or not completed. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `url` | `string` | Public campaign URL. |
| `template` | `Resource<Template>` | Frozen template; unavailable imported resources have explicit empty records. |
| `page` | `Resource<Page>` | Frozen landing page. |
| `smtp` | `Resource<Profile>` | Frozen sender identity with current transport settings, never the stored password. |
| `results` | `Recipient[], optional` | Audience snapshots; an empty audience is omitted in this projection. |
| `timeline` | `Event[], optional` | Committed history; omitted when empty and in creation acknowledgements. |
| `groups` | `Resource<Group> or null[], optional` | Source groups supplied only in creation acknowledgements. |

The response also includes [attribution](conventions.md#attribution). `template`, `page`, and `smtp` use their resource response shapes. Empty `results` and `timeline` are omitted. Direct creation includes `groups` and omits `timeline`; ordinary reads omit `groups`. Authored content and audience are captured for the execution; sending credentials and domain eligibility remain operational dependencies.

### Recipient and timeline fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `string` | Opaque public recipient identifier. |
| `status` | `string` | Current engagement projection, distinct from delivery state. |
| `ip` | `string` | Sanitized historical address. |
| `latitude` | `number` | Sanitized historical latitude. |
| `longitude` | `number` | Sanitized historical longitude. |
| `reported` | `boolean` | Independent reported flag. |
| `send_date` | `string or null` | Scheduled send timestamp, or explicitly unknown. |
| `modified_date` | `string or null` | Last change, or explicitly unknown. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `client` | `Info or null` | Immutable coarse request categories; absent for historical or non-client observations. |
| `campaign_id` | `integer` | Owning execution. |
| `email` | `string` | Administrator-supplied recipient address, empty for campaign-wide events. |
| `time` | `string or null` | Recorded time, or explicitly unknown. |
| `message` | `string` | Recorded event name. |
| `details` | `string` | Sanitized string-encoded metadata; never participant-entered values. |

Recipient rows also contain `email`, `first_name`, `last_name`, and `position`. Their string `id` is the public recipient token, not the numeric row ID used by delivery, history, and result operations. `details` is sanitized JSON encoded inside a string, or an empty string when no details remain.

| Campaign `status` | Meaning |
| --- | --- |
| Created | Created execution. |
| Queued | Waiting for its scheduled launch. |
| In progress | Active execution. |
| Emails Sent | Supported imported lifecycle state. |
| Completed | Closed execution. |

## GET `/api/campaigns/`

List complete execution records. Prefer overview or directory when full content and timelines are unnecessary.

**Request:** No body. **Response:** `200 OK`, `Vec<campaign_details::Campaign>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

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
    "track_opens": false,
    "id": 1,
    "name": "Training campaign",
    "status": "Queued",
    "created_date": "2026-11-03T12:00:00Z",
    "launch_date": "2030-01-01T00:00:00Z",
    "send_by_date": null,
    "completed_date": null,
    "url": "http://127.0.0.1:8081",
    "template": {
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
      "revision": 1,
      "id": 1,
      "name": "Training message",
      "modified_date": "2026-11-03T12:00:00Z",
      "subject": "Training notification",
      "text": "Hello {{.FirstName}}",
      "html": "<p>Hello {{.FirstName}}</p>",
      "envelope_sender": "",
      "attachments": []
    },
    "page": {
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
      "revision": 1,
      "id": 1,
      "name": "Training page",
      "modified_date": "2026-11-03T12:00:00Z",
      "html": "<h1>Training page</h1>",
      "redirect_url": ""
    },
    "smtp": {
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
      "revision": 1,
      "id": 1,
      "name": "Training sender",
      "modified_date": "2026-11-03T12:00:00Z",
      "interface_type": "SMTP",
      "host": "smtp.example.com:587",
      "from_address": "training@example.com",
      "ignore_cert_errors": false,
      "headers": [],
      "username": "training@example.com",
      "has_password": true
    },
    "results": [
      {
        "id": "AbCd1234",
        "email": "ada@example.com",
        "first_name": "Ada",
        "last_name": "Lovelace",
        "position": "Engineer",
        "status": "Scheduled",
        "ip": "",
        "latitude": 0.0,
        "longitude": 0.0,
        "reported": false,
        "send_date": "2030-01-01T00:00:00Z",
        "modified_date": "2026-11-03T12:00:00Z"
      }
    ],
    "timeline": [
      {
        "client": null,
        "campaign_id": 1,
        "email": "",
        "time": "2026-11-03T12:00:00Z",
        "message": "Campaign Created",
        "details": ""
      }
    ]
  }
]
```

## GET `/api/campaigns/{id}`

Read one execution and its recorded audience and timeline.

**Request:** No body. **Response:** `200 OK`, `campaign_details::Campaign` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
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
  "track_opens": false,
  "id": 1,
  "name": "Training campaign",
  "status": "Queued",
  "created_date": "2026-11-03T12:00:00Z",
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "completed_date": null,
  "url": "http://127.0.0.1:8081",
  "template": {
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
    "revision": 1,
    "id": 1,
    "name": "Training message",
    "modified_date": "2026-11-03T12:00:00Z",
    "subject": "Training notification",
    "text": "Hello {{.FirstName}}",
    "html": "<p>Hello {{.FirstName}}</p>",
    "envelope_sender": "",
    "attachments": []
  },
  "page": {
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
    "revision": 1,
    "id": 1,
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<h1>Training page</h1>",
    "redirect_url": ""
  },
  "smtp": {
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
    "revision": 1,
    "id": 1,
    "name": "Training sender",
    "modified_date": "2026-11-03T12:00:00Z",
    "interface_type": "SMTP",
    "host": "smtp.example.com:587",
    "from_address": "training@example.com",
    "ignore_cert_errors": false,
    "headers": [],
    "username": "training@example.com",
    "has_password": true
  },
  "results": [
    {
      "id": "AbCd1234",
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer",
      "status": "Scheduled",
      "ip": "",
      "latitude": 0.0,
      "longitude": 0.0,
      "reported": false,
      "send_date": "2030-01-01T00:00:00Z",
      "modified_date": "2026-11-03T12:00:00Z"
    }
  ],
  "timeline": [
    {
      "client": null,
      "campaign_id": 1,
      "email": "",
      "time": "2026-11-03T12:00:00Z",
      "message": "Campaign Created",
      "details": ""
    }
  ]
}
```

## GET `/api/campaigns/{id}/context`

Read operational context without full content or recipient history.

**Request:** No body. **Response:** `200 OK`, `campaign_reads::Header` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/context" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
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
  "name": "Training campaign",
  "status": "Queued",
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "url": "http://127.0.0.1:8081",
  "template": {
    "id": 1,
    "name": "Training message"
  },
  "page": {
    "id": 1,
    "name": "Training page"
  },
  "smtp": {
    "id": 1,
    "name": "Training sender"
  },
  "subject": "Training notification",
  "profile_sender": "training@example.com",
  "template_sender": "",
  "total": 1,
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  },
  "domain_id": 1,
  "domain_block": null
}
```

### Context fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Execution identity. |
| `name` | `string` | Display name. |
| `status` | `CampaignStatus` | Lifecycle state. |
| `launch_date` | `string or null` | Preserved timestamp. |
| `send_by_date` | `string or null` | Distribution end; null when absent. |
| `url` | `string` | Public campaign URL. |
| `template` | `Resource` | Selected email template. |
| `page` | `Resource` | Selected landing page. |
| `smtp` | `Resource` | Selected sending profile. |
| `subject` | `string or null` | Current subject; null when unavailable. |
| `profile_sender` | `string or null` | Current sender; null when unavailable. |
| `template_sender` | `string or null` | Current override; null when unavailable. |
| `total` | `integer` | Confirmed audience size. |
| `policy` | `Policy or null` | Effective policy; null for unsupported legacy state. |
| `domain_id` | `integer or null` | Bound domain identity. |
| `domain_block` | `string or null` | Current domain eligibility failure. |

Resource references contain numeric `id` and nullable `name`. `policy` contains `track_opens`, `redirect_url`, and optional `learning_page_id`; null means no known historical policy.

## GET `/api/campaigns/overview`

Read a bounded page of execution contexts.

**Request:** Query: campaign::request::OverviewPage. **Response:** `200 OK`, `campaign_reads::Overview` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/overview?after=0&limit=50" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "items": [
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
      "name": "Training campaign",
      "status": "Queued",
      "launch_date": "2030-01-01T00:00:00Z",
      "send_by_date": null,
      "url": "http://127.0.0.1:8081",
      "template": {
        "id": 1,
        "name": "Training message"
      },
      "page": {
        "id": 1,
        "name": "Training page"
      },
      "smtp": {
        "id": 1,
        "name": "Training sender"
      },
      "subject": "Training notification",
      "profile_sender": "training@example.com",
      "template_sender": "",
      "total": 1,
      "policy": {
        "track_opens": false,
        "redirect_url": ""
      },
      "domain_id": 1,
      "domain_block": null
    }
  ],
  "next_after": null,
  "mailer_enabled": true
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `after` | `integer` | Continuation returned by the previous page. |
| `limit` | `integer or null` | Requested page size. |
| `completed` | `boolean or null` | Optional completion filter. |

Pass `next_after` as the next request’s `after`; null ends traversal. `completed=true` selects completed executions, false selects unfinished executions, and omission selects both. `mailer_enabled` reflects startup configuration.

## GET `/api/campaigns/directory`

List saved drafts and executions in one bounded directory.

**Request:** Query: campaign::request::DirectoryPage. **Response:** `200 OK`, `campaign_reads::Directory` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/directory?filter=all&sort=name&order=asc" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "items": [
    {
      "id": "1",
      "revision": null,
      "name": "Training campaign",
      "kind": "campaign",
      "status": "Queued",
      "total": 1,
      "launch_date": "2030-01-01T00:00:00Z"
    }
  ],
  "next_after": null
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| filter | string | `all` (default), `drafts`, `active`, `completed`, or `executions`. |
| sort | string, optional | `recent`, `name`, `status`, `total`, or `launch`. |
| order | string, optional | `asc` (default for explicit sort) or `desc`. Requires `sort`. |
| after | string | Copy `next_after` unchanged and URL-encode it. Keep the caller, filter, sort, and order unchanged. Each page contains at most 50 items. |

Directory items contain a string `id`, `name`, nullable draft `revision`, `kind` (`draft` or `campaign`), and the corresponding `status`. Draft states are `draft` and `launched`; execution states are listed above. `total` and `launch_date` are nullable for drafts.

## GET `/api/campaigns/summary`

List execution metadata and progression counts.

**Request:** No body. **Response:** `200 OK`, `campaign_details::Summaries` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/summary" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "total": 1,
  "campaigns": [
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
      "track_opens": false,
      "id": 1,
      "name": "Training campaign",
      "status": "Queued",
      "created_date": "2026-11-03T12:00:00Z",
      "launch_date": "2030-01-01T00:00:00Z",
      "send_by_date": null,
      "completed_date": null,
      "stats": {
        "total": 1,
        "sent": 0,
        "opened": 0,
        "clicked": 0,
        "submitted_data": 0,
        "email_reported": 0,
        "error": 0
      }
    }
  ]
}
```

## GET `/api/campaigns/{id}/summary`

Read metadata and progression counts for one execution.

**Request:** No body. **Response:** `200 OK`, `campaign_details::Summary` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/summary" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
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
  "track_opens": false,
  "id": 1,
  "name": "Training campaign",
  "status": "Queued",
  "created_date": "2026-11-03T12:00:00Z",
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "completed_date": null,
  "stats": {
    "total": 1,
    "sent": 0,
    "opened": 0,
    "clicked": 0,
    "submitted_data": 0,
    "email_reported": 0,
    "error": 0
  }
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `total` | `integer` | Audience size. |
| `sent` | `integer` | Recorded SMTP acceptances. |
| `opened` | `integer` | Cumulative opens. |
| `clicked` | `integer` | Cumulative clicks. |
| `submitted_data` | `integer` | Recorded submissions. |
| `email_reported` | `integer` | Independent reports. |
| `error` | `integer` | Failed recipients. |

These summary counts represent recipient progression: a submission contributes to clicked and opened counts, and a click contributes to opened. SMTP acceptance is never inferred. Use [results](analysis-and-exports.md) for separate observed totals and unique counts.

## GET `/api/campaigns/{id}/draft`

Return editable duplication input without persisting a saved draft.

**Request:** No body. **Response:** `200 OK`, `campaign_workflow::Duplicate` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/draft" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "name": "Training campaign (copy)",
  "url": "http://127.0.0.1:8081",
  "selection": {
    "groups": [
      {
        "id": 1
      }
    ],
    "template": {
      "id": 1
    },
    "page": {
      "id": 1
    },
    "smtp": {
      "id": 1
    }
  },
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  }
}
```

The duplication response contains `name`, `url`, ID-only `selection`, and nullable `policy`. It carries no schedule or reviewed revisions; prepare the new configuration before creating or launching it.

## POST `/api/campaigns/test`

Send a prepared sample to one explicit recipient without creating an execution.

**Request:** campaign::Test. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

`campaign` uses the command above. `email` is the test destination; rendering uses Sample Recipient as the recipient name. Success means SMTP acceptance, not inbox delivery.

Request body (`request.json`):

```json
{
  "campaign": {
    "name": "Training campaign",
    "url": "http://127.0.0.1:8081",
    "selection": {
      "groups": [
        {
          "id": 1,
          "revision": "1"
        }
      ],
      "template": {
        "id": 1,
        "revision": "1"
      },
      "page": {
        "id": 1,
        "revision": "1"
      },
      "smtp": {
        "id": 1,
        "revision": "1"
      }
    },
    "launch_date": "2030-01-01T00:00:00Z",
    "send_by_date": null,
    "policy": {
      "track_opens": false,
      "redirect_url": ""
    },
    "launch_local": null,
    "send_by_local": null
  },
  "email": "tester@example.com"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/test" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "success": true,
  "message": "Accepted by SMTP server",
  "data": null
}
```

## POST `/api/campaigns/{id}/complete`

Close the execution and stop pending delivery work.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/1/complete" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "success": true,
  "message": "Campaign completed successfully!",
  "data": null
}
```

## DELETE `/api/campaigns/{id}`

Delete the execution and its retained campaign data.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/campaigns/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "success": true,
  "message": "Campaign deleted successfully!",
  "data": null
}
```
