---
title: Groups API
description: Create and maintain recipient groups and inspect their summaries.
sidebar:
  order: 5
---

A group contains campaign recipients. It does not create application accounts or grant access to Ruphish.

| Method | Path | Result |
| --- | --- | --- |
| GET | `/api/groups/` | Array of owned groups |
| POST | `/api/groups/` | Create a group; return the saved object |
| GET | `/api/groups/{id}` | Read a group and its ETag |
| PUT | `/api/groups/{id}` | Replace a group; include the matching body ID |
| DELETE | `/api/groups/{id}` | Delete an eligible group |
| GET | `/api/groups/summary` | Collection counts and group summaries |
| GET | `/api/groups/{id}/summary` | One group summary |

## Read a group

Set the connection variables from [your first API request](index.md#make-your-first-request), then replace the dummy ID with a group ID returned by the collection:

```sh
export GROUP_ID='42'

curl --header "Authorization: Bearer $RUPHISH_API_KEY" \
  "$RUPHISH_URL/api/groups/$GROUP_ID"
```

The response contains the owned group. Include the private-CA option from the setup example if your installation requires it.

## Create a group

Send this body to `POST /api/groups/`:

```json
{
  "name": "Local tutorial",
  "targets": [
    {
      "email": "alex@example.test",
      "first_name": "Alex",
      "last_name": "Example",
      "position": "Support"
    }
  ]
}
```

The saved object adds `id` and `modified_date`. Target fields are `email`, `first_name`, `last_name`, and `position`; an email address must pass validation. Treat identity and duplicate behavior as part of the import or launch operation, not as a promise that every possible duplicate is removed by a resource write.

The collection summary has `total` and `groups`. Each summary includes `id`, `name`, `modified_date`, and `num_targets`.

Use [CSV import review](imports-and-previews.md) to show row-level errors before saving an imported group. See [the group guide](../user-guide/groups.md) for CSV encoding, headers, and duplicate handling, and [conventions](conventions.md) for conditional updates.
