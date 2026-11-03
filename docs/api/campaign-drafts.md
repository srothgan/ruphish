---
title: Campaign drafts API
description: Save incomplete campaign work with revision checks and launch it once.
sidebar:
  order: 11
---

Draft persistence does not send mail, reserve recipients, or prove a campaign is ready to launch. It stores editable configuration and the current wizard step.

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/campaign-drafts` | Page through drafts with numeric `after` |
| POST | `/api/campaign-drafts` | Create a draft with an `Idempotency-Key` header |
| POST | `/api/campaign-drafts/prepare` | Review configuration using draft scheduling rules |
| GET | `/api/campaign-drafts/{id}` | Read a saved draft |
| PUT | `/api/campaign-drafts/{id}` | Update an existing draft |
| DELETE | `/api/campaign-drafts/{id}?revision={revision}` | Delete the expected revision |
| POST | `/api/campaign-drafts/{id}/launch` | Launch a reviewed draft |

## Save incomplete work

Choose a random draft ID of 16–64 ASCII letters, digits, or hyphens. Send this body to its PUT route to create an empty draft:

```json
{
  "configuration": {
    "name": "Local exercise",
    "url": "",
    "selection": {
      "groups": [],
      "template": {"id": 0},
      "page": {"id": 0},
      "smtp": {"id": 0}
    },
    "policy": null,
    "launch_local": null,
    "send_by_local": null
  },
  "step": 0
}
```

The response contains `id`, `revision`, `configuration`, `step`, creation/modification timestamps, optional `campaign_id`, and `status` (`draft` or `launched`). Save the returned numeric ID and revision. Update with `PUT /api/campaign-drafts/{id}`, adding `"revision": 1` (or the latest saved revision) to the body. Updates return `200`; a missing revision returns `428`, and PUT never creates a missing draft. Steps range from 0 through 3. A stale edit fails instead of overwriting another editor's work.

Creation keys must be nonempty and at most 128 bytes; malformed keys return `400` with `IDEMPOTENCY_KEY_INVALID`. Retain the same key and original body when retrying a lost response. The key is scoped to the authenticated caller and draft creation, independently of direct campaign creation. Replaying the same request returns the existing draft's current state, including any later edits or launch reference; reusing its key with changed input returns `409`. A retry after deletion returns `404` and cannot recreate the draft. Use a new key for a separate creation. Updates retain the existing preceding-revision retry check for unchanged content and step.

Configuration is a closed schema: unknown fields are rejected. Resource references contain IDs only; do not embed resource bodies, recipients, passwords, or API keys. Zero means unselected. The name is limited to 255 characters, URL and redirect to 8,192, and group selections to 1,000. Local schedule objects contain `local` and `time_zone`.

List responses contain `items` and numeric/null `next_after`; pages contain at most 50 drafts. Continue only when a next cursor is present. The combined campaign directory retains its decimal-string IDs for both drafts and executions.

## Launch the reviewed revision

The launch body contains `revision` and `campaign`, where `campaign` is the full reviewed [native campaign request](campaigns.md#prepare-and-launch). It must match the saved draft configuration. Submit resource IDs and reviewed revisions, without the display names returned by preparation. Unknown and duplicate nested fields are rejected before launch. Preparation resolves current resources and catches schedules that are no longer in the future.

Launch ties the draft to one execution. If the response is lost, retrying the same draft launch returns that execution rather than creating another. The draft identity owns this behavior; an unrelated idempotency header cannot bypass it. After launch, use the execution APIs for delivery and results.
