---
title: Import a website
description: Migrate POST /api/import/site to the current Ruphish contract.
sidebar:
  order: 182
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Retain the import operation while adapting validation and response handling.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/import/site` | `POST /api/import/site` |
| Success | `200 OK` | `200 OK` |
| Response type | `cloneResponse` | `resources::HtmlImport` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `url` | String destination. | Same field; subject to outbound destination policy and certificate verification. |
| `include_resources` | Accepted boolean; the pinned handler does not implement resource embedding. | Not part of the request contract. Remove it; use the new ZIP page-bundle importer for local bundled assets. |
| Response `html` | HTML string with a base URL and original-form URL metadata. | Same outer shape; external resources remain external references. |
| TLS/network behavior | The pinned site importer disabled certificate verification. | Current trust and outbound-network policy applies. Fix trust/destination configuration rather than assuming an old import will succeed. |

## Gophish request

`POST /api/import/site`

```json
{
  "url": "https://pages.example.com/training",
  "include_resources": false
}
```

## Gophish response

```json
{
  "html": "<html><head><base href=\"https://pages.example.com/training\"/></head><body><h1>Training</h1></body></html>"
}
```

## Ruphish request

Request type: `resources::request::SiteRequest`. Save this body as `request.json`:

```json
{
  "url": "https://pages.example.com/training"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/import/site" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

## Ruphish response

```json
{
  "html": "<html><head><base href=\"https://pages.example.com/training\"></head><body><h1>Training</h1></body></html>"
}
```

## Behavior and verification

Requires `modify_objects`. The response example assumes the fetched document contains a Training heading and no existing base tag or forms. Fetching a page does not save a landing-page resource or establish that external assets will remain available.

Test representative valid content and a malformed input in isolation. Inspect the returned content before saving it as a shared resource.

Current contract: [Import a website](../../../api/imports-and-previews.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/import.go).
