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
| POST | `/api/pages/preview` | Prepared page `html` and advisory `warnings` |
| POST | `/api/pages/validate` | Advisory `warnings` for supplied `html` |

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

The saved object adds `id`, `modified_date`, and `warnings`. Updates require the matching body ID and support an ETag precondition.

## Legacy flags do not enable value capture

The field names remain for compatibility. Both flags are ignored on input and returned as `true`; every supported submission records an event and password presence. Neither pages nor campaign overrides can disable tracking or enable stored usernames, passwords, OTPs, arbitrary field values, or value derivatives.

The authored HTML and the served participant page are different representations. Ruphish applies restrictions to served content and uses a controlled event submission path. Imported scripts, external resources, and form behavior may therefore differ from the source website. Read [landing pages](../user-guide/landing-pages.md) and [participant data](../user-guide/participant-data.md), including the current verification limits.

`redirect_url` controls the destination after a tracked submission. Use a supported HTTP(S) or relative destination. An empty value leaves the participant on the page. This API is for administrative page management; participant interaction endpoints are not a general-purpose form collection API.

## Advisory validation

`POST /api/pages/validate` accepts `{"html":"<form>...</form>"}` and requires the same authenticated editing permission as saving a page. It returns `{"warnings":[{"code":"password_disabled","control":1}]}`. `control` is an optional one-based input index, never a field value. Codes are `password_disabled`, `password_unnamed`, `password_without_form`, `form_without_submit`, `duplicate_control_id`, `html_repaired`, and `authored_execution_removed`. Invalid template syntax still rejects the request; advisory findings do not block saving. Page reads, saves, `POST /api/util/preview`, and campaign review include warnings. Diagnostics use a rendered sample and do not replace representative recipient/manual browser checks.

## Preview a landing page

`POST /api/pages/preview` accepts `html` and an optional `recipient` object with `email`, `first_name`, `last_name` and `position`. Omitted recipient fields are empty strings. It returns `200` with `{"html":"...","warnings":[]}` and does not save content or create campaign events. The same editing permission and session-CSRF rules apply as for saving pages; malformed requests and invalid template expressions return `400`.

The preview expands compatible template expressions using the supplied recipient, then uses the same document preparation and sanitizer as public pages. Authored scripts, unsafe attributes and remote resources are removed. Forms retain their structure, but the preview contains no submission collector. The document title stays in the HTML head, and semantic layout elements such as `main` and `section` are preserved. When embedding the returned HTML, use a sandboxed frame that blocks scripts and form submissions, as the admin UI does.

Use this endpoint for landing-page previews. The legacy [`/api/util/preview`](imports-and-previews.md#preview-html) endpoint retains template/email-preview behavior; it is not the restricted public-page representation. [Learning-page previews](learning-pages.md) use the same page renderer in static mode without recipient expansion or form controls.
