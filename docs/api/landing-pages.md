---
title: Landing pages API
description: Manage campaign pages, redirects and event-only interaction tracking.
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
  "redirect_url": ""
}
```

The saved object adds `id`, `modified_date`, and `warnings`. Updates require the matching body ID and support an ETag precondition.

## Interaction tracking

The API has no capture settings. Every supported submission records an event and password presence. Neither pages nor campaign overrides can disable this built-in tracking or enable stored usernames, passwords, OTPs, arbitrary field values, or value derivatives. Responses omit `capture_credentials` and `capture_passwords`.

The served page preserves authored scripts, event handlers and visual assets while enforcing managed native submissions and an opaque-origin sandbox. Custom code can make independent requests; it is outside the built-in presence-only guarantee. See [landing pages](../user-guide/landing-pages.md) for supported content, asset imports and `window.ruphish.submit(form)`, and [participant data](../user-guide/participant-data.md) for the privacy boundary.

`redirect_url` controls the destination after a tracked submission. Use a supported HTTP(S) or relative destination. An empty value leaves the participant on the page. This API is for administrative page management; participant interaction endpoints are not a general-purpose form collection API.

## Advisory validation

`POST /api/pages/validate` accepts `{"html":"<form>...</form>"}` and requires the same authenticated editing permission as saving a page. It returns `{"warnings":[{"code":"password_disabled","control":1}]}`. `control` is an optional one-based input index, never a field value. Landing-page codes are `password_disabled`, `password_unnamed`, `password_without_form`, `form_without_submit`, `duplicate_control_id` and `html_repaired`; `authored_execution_removed` remains applicable to static learning pages only. Invalid template syntax still rejects the request; advisory findings do not block saving. Page reads, saves, `POST /api/util/preview`, and campaign review include warnings. Diagnostics use a rendered sample and do not certify scripts or replace representative recipient/manual browser checks.

## Preview a landing page

`POST /api/pages/preview` accepts `html` and an optional `recipient` object with `email`, `first_name`, `last_name` and `position`. Omitted recipient fields are empty strings. It returns `200` with `{"html":"...","warnings":[]}` and does not save content or create campaign events. The same editing permission and session-CSRF rules apply as for saving pages; malformed requests and invalid template expressions return `400`.

The preview expands compatible template expressions using the supplied recipient and uses the same document preparation as public pages. Authored scripts, event handlers and remote visual resources remain. Its collector has no event endpoint: submission shows preview feedback without campaign events or redirects. External authored requests remain possible. The first document title stays in the head, and semantic layout elements are preserved.

Embed the returned HTML only in an iframe with `sandbox="allow-scripts allow-forms"` and `referrerpolicy="no-referrer"`, preserving its generated content policy. Never add `allow-same-origin`, top-navigation or popup permissions. `allow-forms` permits native validation and submit events; CSP `form-action 'none'` blocks native network submissions. Do not insert the HTML into the administrative document or serve it as unrestricted same-origin content. Public landing responses additionally enforce the sandbox in their HTTP CSP header.

Use this endpoint for landing-page previews. The legacy [`/api/util/preview`](imports-and-previews.md#preview-html) endpoint retains template/email-preview behavior; it is not the restricted public-page representation. [Learning-page previews](learning-pages.md) use the same page renderer in static mode without recipient expansion or form controls.
