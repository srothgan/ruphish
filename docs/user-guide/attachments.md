---
title: Add and personalize attachments
description: Attach files to email templates and understand supported document personalization.
sidebar:
  order: 6
---

Add attachments to an email template, then save the template before using it in a campaign. A raw-email import does not automatically recover MIME attachments, so inspect the imported template and upload the files you need.

## Supported personalization

| Filename extension | Processing |
| --- | --- |
| `.txt`, `.html`, `.ics` | Interpret UTF-8 text using the template context |
| `.docx`, `.docm`, `.pptx`, `.xlsx`, `.xlsm` | Render supported expressions in XML/relationship entries inside the Office archive |
| Other extensions | Preserve the file bytes without template processing |

Extension matching is case-sensitive in the attachment processor; use the lowercase forms above. Filename extensions do not make arbitrary content a valid Office archive.

For a text attachment, you can start with:

```text
Training note for {{.FirstName}} {{.LastName}}
Open your local exercise: {{.URL}}
```

Download/open the received attachment from a test message and verify the actual rendered content. Office editors can split visible text across XML runs; checking what the recipient receives is more useful than only inspecting the source document visually.

## Tracking limits

A document containing a linked tracking image may request that image when opened, depending on the document viewer and its settings. That request uses the same tracking mechanism as an email image. It does not reliably establish which application displayed the content or whether a human enabled a particular document feature.

Do not interpret a missing request as proof the attachment was unopened. Link scanning, blocked remote content, and cached images affect these observations.

## Size and format limits

Attachments are limited to 16 MiB decoded content; the resource request has a separate 32 MiB limit including base64/JSON overhead. Office archives are limited to 4,096 entries and 64 MiB expanded content, with 8 MiB per processed XML part. Invalid, unsafe, or oversized archive content fails rather than producing a partially rendered attachment.

API clients supply base64 content with the attachment's name and MIME type. See [templates API](../api/templates.md).
