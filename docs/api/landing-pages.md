---
title: Landing pages API
description: Manage campaign pages and understand the privacy meaning of legacy capture flags.
sidebar:
  order: 7
---

| Method | Path | Result |
| --- | --- | --- |
| GET, POST | `/api/pages/` | List or create landing pages |
| GET, PUT, DELETE | `/api/pages/{id}` | Read, replace, or delete a page |
| GET | `/api/pages/summary` | `total` and resource `items` |

## Create a page

```json
{
  "name": "Local exercise",
  "html": "<h1>Exercise complete</h1><p>This is a local demonstration.</p>",
  "capture_credentials": false,
  "capture_passwords": false,
  "redirect_url": ""
}
```

The saved object adds `id` and `modified_date`. Updates require the matching body ID and support an ETag precondition.

## Legacy flags do not enable value capture

The field names remain for compatibility. `capture_credentials` controls submission-event tracking; `capture_passwords` permits a password-presence boolean. Neither flag permits stored usernames, passwords, OTPs, arbitrary field values, or value derivatives. Campaign policy can override these choices for an execution.

The authored HTML and the served participant page are different representations. Ruphish applies restrictions to served content and uses a controlled event submission path. Imported scripts, external resources, and form behavior may therefore differ from the source website. Read [landing pages](../user-guide/landing-pages.md) and [participant data](../user-guide/participant-data.md), including the current verification limits.

`redirect_url` controls the destination after a tracked submission. Use a supported HTTP(S) or relative destination. An empty value leaves the participant on the page. This API is for administrative page management; participant interaction endpoints are not a general-purpose form collection API.
