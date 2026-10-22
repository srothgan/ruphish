---
title: Campaign drafts API
description: Save incomplete campaign work with revision checks and launch it once.
sidebar:
  order: 11
---

Draft persistence does not send mail, reserve recipients, or prove a campaign is ready to launch. It stores editable configuration and the current wizard step.

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/campaign-drafts` | Page through drafts with `after` |
| POST | `/api/campaign-drafts/prepare` | Review configuration using draft scheduling rules |
| GET | `/api/campaign-drafts/{id}` | Read a saved draft |
| PUT | `/api/campaign-drafts/{id}` | Create or update a draft |
| DELETE | `/api/campaign-drafts/{id}?revision={revision}` | Delete the expected revision |
| POST | `/api/campaign-drafts/{id}/launch` | Launch a reviewed draft |

## Save incomplete work

Choose a random draft ID of 16–64 ASCII letters, digits, or hyphens. Send this body to its PUT route to create an empty draft:

```json
{
  "revision": null,
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

The response contains `id`, `revision`, `configuration`, `step`, creation/modification timestamps, optional `campaign_id`, and `status` (`Draft` or `Launched`). Save the returned revision and send it with the next edit. Steps range from 0 through 3. A stale edit fails instead of overwriting another editor's work.

Configuration is a closed schema: unknown fields are rejected. Resource references contain IDs only; do not embed resource bodies, recipients, passwords, or API keys. Zero means unselected. The name is limited to 255 characters, URL and redirect to 8,192, and group selections to 1,000. Local schedule objects contain `local` and `time_zone`.

List responses contain `items` and `next_after`; pages contain at most 50 drafts. Continue only when a next cursor is present.

## Launch the reviewed revision

The launch body contains `revision` and `campaign`, where `campaign` is the full reviewed [campaign request](campaigns.md). It must match the saved draft configuration. Preparation resolves current resources and catches schedules that are no longer in the future.

Launch ties the draft to one execution. If the response is lost, retrying the same draft launch returns that execution rather than creating another. The draft identity owns this behavior; an unrelated idempotency header cannot bypass it. After launch, use the execution APIs for delivery and results.
