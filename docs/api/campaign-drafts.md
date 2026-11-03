---
title: Campaign drafts
description: Draft persistence, revisions, preparation, and idempotent launch.
sidebar:
  order: 21
---

Draft persistence accepts incomplete configuration and does not authorize mail delivery. Reads require `view_objects`; saving, preparing, deleting, and launching require `modify_objects`. The body limit is 1 MiB.

## Configuration fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `name` | `string` | Campaign display name; empty is permitted until launch. |
| `url` | `string` | Public destination; empty is permitted until launch. |
| `selection` | `Selections` | Resource references only. |
| `policy` | `Policy or null` | None means page defaults have not been resolved yet. |
| `launch_local` | `LocalTime or null` | None requests immediate delivery at launch. |
| `send_by_local` | `LocalTime or null` | Optional end of initial distribution. |

`selection` contains ordered `groups` plus `template`, `page`, and `smtp`, each as `{ "id": 1 }`. Zero means unselected; negative IDs are invalid. Draft input defaults omitted fields, but responses always contain the complete configuration. `policy` is nullable; when present it contains `track_opens`, nullable `redirect_url`, and optional `learning_page_id`. A null redirect means unresolved defaults. Local time objects contain `local` and `time_zone` strings, which may still be incomplete. Unknown fields are rejected.

| Attribute | Type | Contract |
| --- | --- | --- |
| step | integer | Editing position, 0 through 3. |
| revision | integer | Current saved revision; required for replacement, deletion, and launch. |
| configuration | object | Name up to 255 characters; URL and redirect up to 8,192; at most 1,000 group references. Saving incomplete values does not make them launchable. |

## Draft response

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Database-assigned draft identity. |
| `revision` | `integer` | Conditional-write revision. |
| `configuration` | `Configuration` | Complete saved structure whose values may still be unfinished; saving never authorizes launch. |
| `step` | `integer` | Last editing step. |
| `created` | `string or null` | UTC creation timestamp. |
| `modified` | `string or null` | UTC modification timestamp. |
| `campaign_id` | `integer or null` | Launched execution identity, if any. |
| `status` | `campaign_draft::Status` | Draft or launched. |

Attribution follows the [shared response fields](conventions.md#attribution). `status` is `draft` when `campaign_id` is null and `launched` after execution creation.

## POST `/api/campaign-drafts`

Persist an editable configuration.

**Request:** campaign::request::DraftCreate. **Response:** `201 Created`, `campaign_reads::Draft` as `application/json`.

An Idempotency-Key is required. Reuse it only for a retry of the same typed creation request; use a fresh key for a distinct draft.

Request body (`request.json`):

```json
{
  "configuration": {
    "name": "Training campaign",
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
    },
    "launch_local": {
      "local": "2030-01-01T00:00",
      "time_zone": "UTC"
    },
    "send_by_local": null
  },
  "step": 3
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaign-drafts" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'Idempotency-Key: draft-training-2030-01' \
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
  "id": 1,
  "revision": 1,
  "configuration": {
    "name": "Training campaign",
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
    },
    "launch_local": {
      "local": "2030-01-01T00:00",
      "time_zone": "UTC"
    },
    "send_by_local": null
  },
  "step": 3,
  "created": "2026-11-03T12:00:00Z",
  "modified": "2026-11-03T12:00:00Z",
  "campaign_id": null,
  "status": "draft"
}
```

## GET `/api/campaign-drafts`

List at most 50 saved drafts after the numeric cursor.

**Request:** Query: campaign::request::DraftPage. **Response:** `200 OK`, `campaign_reads::Drafts` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaign-drafts?after=0" \
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
      "revision": 1,
      "configuration": {
        "name": "Training campaign",
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
        },
        "launch_local": {
          "local": "2030-01-01T00:00",
          "time_zone": "UTC"
        },
        "send_by_local": null
      },
      "step": 3,
      "created": "2026-11-03T12:00:00Z",
      "modified": "2026-11-03T12:00:00Z",
      "campaign_id": null,
      "status": "draft"
    }
  ],
  "next_after": null
}
```

`after` defaults to zero and cannot be negative. Pass `next_after` to continue; null ends the list. Unknown query fields are rejected.

## GET `/api/campaign-drafts/{id}`

Read a saved configuration.

**Request:** No body. **Response:** `200 OK`, `campaign_reads::Draft` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaign-drafts/1" \
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
  "revision": 1,
  "configuration": {
    "name": "Training campaign",
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
    },
    "launch_local": {
      "local": "2030-01-01T00:00",
      "time_zone": "UTC"
    },
    "send_by_local": null
  },
  "step": 3,
  "created": "2026-11-03T12:00:00Z",
  "modified": "2026-11-03T12:00:00Z",
  "campaign_id": null,
  "status": "draft"
}
```

## PUT `/api/campaign-drafts/{id}`

Replace configuration and editing step at the supplied revision.

**Request:** campaign::request::DraftWrite. **Response:** `200 OK`, `campaign_reads::Draft` as `application/json`.

A retry with the preceding revision and identical saved configuration and step returns the saved draft. Other stale writes fail with `412`. Launched drafts cannot be edited.

Request body (`request.json`):

```json
{
  "revision": 1,
  "configuration": {
    "name": "Training campaign",
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
    },
    "launch_local": {
      "local": "2030-01-01T00:00",
      "time_zone": "UTC"
    },
    "send_by_local": null
  },
  "step": 3
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/campaign-drafts/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
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
  "id": 1,
  "revision": 2,
  "configuration": {
    "name": "Training campaign",
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
    },
    "launch_local": {
      "local": "2030-01-01T00:00",
      "time_zone": "UTC"
    },
    "send_by_local": null
  },
  "step": 3,
  "created": "2026-11-03T12:00:00Z",
  "modified": "2026-11-03T12:00:00Z",
  "campaign_id": null,
  "status": "draft"
}
```

## POST `/api/campaign-drafts/prepare`

Review launch input for a draft without saving or launching it.

**Request:** campaign::Create. **Response:** `200 OK`, `campaign_workflow::Review` as `application/json`.

This endpoint takes a complete campaign command, not a saved draft ID. It rejects a resolved launch time in the past. Review fields have the same meaning as [campaign preparation](campaigns.md).

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
curl --request POST "$RUPHISH_URL/api/campaign-drafts/prepare" \
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

## POST `/api/campaign-drafts/{id}/launch`

Launch the unchanged saved configuration at its reviewed revision.

**Request:** campaign::LaunchDraft. **Response:** `200 OK`, `campaign_details::Campaign` as `application/json`.

Every selected resource needs its reviewed revision. The command must match the saved configuration. Draft identity owns launch idempotency; repeated launch resolves to its committed execution, and an independent Idempotency-Key does not create another execution.

Request body (`request.json`):

```json
{
  "revision": 2,
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
  }
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaign-drafts/1/launch" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
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

## DELETE `/api/campaign-drafts/{id}`

Remove an editable draft at the supplied query revision.

**Request:** Query: common::Revision. **Response:** `200 OK`, `campaign_reads::DraftRemoval` as `application/json`.

This independent example assumes the draft has not been launched. The precondition is a query parameter, not If-Match or a JSON body.

```sh
curl --request DELETE "$RUPHISH_URL/api/campaign-drafts/1?revision=2" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "success": true
}
```
