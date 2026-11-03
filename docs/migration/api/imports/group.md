---
title: Import recipient CSV
description: Migrate POST /api/import/group to the current Ruphish contract.
sidebar:
  order: 180
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Retain the import operation while adapting validation and response handling.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/import/group` | `POST /api/import/group` |
| Success | `200 OK` | `200 OK` |
| Response type | `[]models.Target` | `Vec<resource_details::Recipient>` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Input | Multipart CSV file; header matching and row parsing. | Same media type and recipient fields. Files have an 8 MiB content bound and at most 100,000 data rows. |
| Invalid email rows | Parser could skip invalid addresses. | Invalid rows fail the import with 400 `CSV_ROWS_INVALID`; use the new review endpoint for per-row errors. |
| Output | Array of `email`, `first_name`, `last_name`, `position` strings. | Same shape. The import does not create a saved group. |
| Method | Legacy handler did not constrain its method. | Use POST. |

## Gophish request

`POST /api/import/group`

Upload a file part named `file` in `multipart/form-data`, with a filename such as `recipients.csv`:

```csv
Email,First Name,Last Name,Position
ada@example.com,Ada,Lovelace,Engineer
```

## Gophish response

```json
[
  {
    "email": "ada@example.com",
    "first_name": "Ada",
    "last_name": "Lovelace",
    "position": "Engineer"
  }
]
```

## Ruphish request

Request type: ``multipart/form-data``. Upload the same CSV as multipart form data. The file part must have a filename.

```sh
curl --request POST "$RUPHISH_URL/api/import/group" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --form "file=@recipients.csv;type=text/csv"
```

## Ruphish response

```json
[
  {
    "email": "ada@example.com",
    "first_name": "Ada",
    "last_name": "Lovelace",
    "position": "Engineer"
  }
]
```

## Behavior and verification

Requires `modify_objects`. Save the returned recipients through the group create/update operation after reviewing them. The new JSON CSV review endpoint also supports explicit column mapping.

Test representative valid content and a malformed input in isolation. Inspect the returned content before saving it as a shared resource.

Current contract: [Import recipient CSV](../../../api/imports-and-previews.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/import.go).
