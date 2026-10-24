---
title: Starter templates API
description: Read the bundled catalog and create ordinary independent resource copies.
sidebar:
  order: 8
---

These read-only endpoints require an authenticated administrator with `modify_system`. Browser sessions cannot use them during impersonation. API keys and browser-session authentication follow the existing [authentication rules](authentication.md).

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/starter-templates/` | `200`: JSON array of entry summaries |
| GET | `/api/starter-templates/{id}` | `200`: `{ "summary": { ... }, "resource": { ... } }` |

There is no pagination or remote catalog request. Responses use `Cache-Control: no-store`. Missing authentication returns `401`; insufficient permissions or an impersonated session returns `403`. An unknown ID returns `404` with `RESOURCE_NOT_FOUND`. Mutation methods are unsupported and return `405`.

Each summary contains:

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | Catalog lookup key, for example `page-microsoft-side-panel` |
| `name` | string | Suggested resource name |
| `brand` | string | Searchable brand |
| `kind` | string | `email` or `landing_page` |
| `prefill` | boolean | Whether the entry supports recipient-email prefill |
| `company_logo` | object or null | Reviewed logo replacement metadata: `selector` (unique CSS selector), `height` (maximum CSS pixels), and `context` (vendor branding context) |

Capabilities do not enable options automatically. The supplied `resource` contains ordinary create fields with standard branding and an empty email input on landing pages. It contains no resource ID, owner, catalog ID or installation state. Email resources contain `name`, `html`, `subject`, `text`, and `attachments`; landing pages contain `name`, `html`, and `redirect_url`.

Read the desired entry, take its `resource` object, choose a unique name, and POST it to [`/api/templates/`](templates.md) or [`/api/pages/`](landing-pages.md). Creation uses the same validation, ownership and browser CSRF checks as other resources and returns `201`. Send only the ordinary resource payload, not the catalog envelope. There is no separate import endpoint or catalog relationship after creation.

For example, read `GET /api/starter-templates/email-microsoft-shared-document`, then submit the returned `resource` object to `POST /api/templates/`. The executable catalog test exercises this read/create flow for every bundled entry, including browser CSRF rejection, independent copies and owner isolation.

Clients that apply options must transform a copy before posting. Prefill uses the supported `{{.Email | html}}` expression; custom logos become embedded image data. The admin UI performs these transformations and validates/rasterizes uploads. The general resource API remains responsible for validating the resulting document. Creation requests are not automatically retried or grouped into a transaction; after an ambiguous response, inspect existing resources before creating another copy.

Catalog content is versioned with the application and can change when the application is updated. Never store the catalog lookup key as a resource relationship. See the [selection workflow and supported branding layouts](../user-guide/starter-templates.md).
