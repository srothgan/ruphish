---
title: Learning pages API
description: Manage reusable static explanations and select campaign redirect destinations.
sidebar:
  order: 8
---

Administrative requests use the existing [authentication](authentication.md), editing permissions and session-CSRF rules. The public HTML route is separate from the management API.

| Method | Path | Result |
| --- | --- | --- |
| GET, POST | `/api/learning-pages/` | List shared pages or create a page |
| GET, PUT, DELETE | `/api/learning-pages/{id}` | Read, replace or delete a shared page |
| GET | `/api/learning-pages/summary` | `total` and metadata `items`, including `active_campaigns` |
| POST | `/api/learning-pages/preview` | Restricted static `html` and advisory `warnings` |
| POST | `/api/learning-pages/validate` | The same processed HTML and diagnostics |
| GET, HEAD | `/learn/{public-page-id}` on the public listener | Restricted HTML without authentication or event recording |

## Resource representation

Create a page with `{"name":"Explanation","html":"<h1>Check the sender</h1><p>Inspect unexpected requests before acting.</p>"}`. Creation returns HTTP 201; reads and replacements return HTTP 200. The resource adds `id`, `revision`, `modified_date` and the server-assigned `public_id`. Reads and saves include `warnings`; individual reads also include `preview_html` for the restricted static representation. Lists return authored resource data.

Updates require the matching body `id`. Use the returned ETag with `If-Match` to prevent stale replacement; a stale revision returns 412. The public identifier remains stable across edits and cannot be selected or replaced by request fields. Importing or duplicating content creates a new resource identity.

An unfinished campaign reference prevents deletion with HTTP 409. New names must be unique within the resource family across the installation. Colleagues can select, read, change and delete resources subject to the same permissions and lifecycle checks.

Preview and validation accept `{"html":"<h1>Explanation</h1>"}`. Warning codes include `learning_controls_removed`, `authored_execution_removed` and `html_repaired`. Warnings are advisory; malformed request data or oversized documents still fail validation. HTML is static: Go-template expressions remain literal, and no recipient values are supplied. Forms, input controls and authored execution are removed or blocked under the public content policy.

## Campaign selection

Use `"policy":{"learning_page_id":123}` in a [campaign prepare/create request](campaigns.md). The ID must identify an existing shared learning page. It overrides the landing page's redirect default. A nonempty `redirect_url` alongside `learning_page_id` is rejected. To select a custom destination, omit `learning_page_id` and supply `redirect_url`; an explicit empty string selects no redirect. Legacy requests that omit both retain landing-page default resolution.

Effective-policy responses include `learning_page_id` when selected and an empty `redirect_url`. Drafts can retain an incomplete learning selection using ID `0`; preparation and launch reject it. If the selected page is deleted before launch, select an existing shared page before retrying.

After an acknowledged submission, the existing participant response returns the generated absolute URL in `redirect`. The URL uses a new opaque identity for the campaign's frozen learning content, distinct from the resource's standalone `public_id`; it contains no recipient ID. Later source edits do not change this content. It remains available after completion while the campaign domain is eligible, and is removed when the campaign is deleted. GET and HEAD do not record an event. Other methods return 405, and missing identities or ineligible origins return 404. Content uses `Cache-Control: no-store`, `Referrer-Policy: no-referrer` and a restrictive content-security policy. A later failed page request does not undo the accepted submission.

PUT and DELETE require the returned quoted numeric ETag in `If-Match`. Missing preconditions return `428`; stale revisions return `412` without changing saved data. See [conditional resource writes](conventions.md#updating-a-resource) for the full contract and conflict recovery.
