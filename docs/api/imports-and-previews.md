---
title: Imports, previews, and test sends
description: Validate CSV, extract email content, import a page, and test rendering or SMTP.
sidebar:
  order: 9
---

These operations help prepare resources. An import or preview does not save a reusable resource unless you subsequently call its create/update endpoint. They require permission to modify objects.

## CSV recipients

`POST /api/import/group` accepts multipart uploaded file parts and returns a recipient array. It rejects a file with invalid rows. For an interface that lets a person inspect and correct rows, use `POST /api/import/group/review` with JSON:

```json
{
  "content": "Email,First Name,Last Name,Position\nalex@example.test,Alex,Example,Support\n",
  "columns": ["Email", "First Name", "Last Name", "Position"]
}
```

`columns` is optional; when supplied, the four entries map email, first name, last name, and position in that order. The response contains accepted `targets`, detected/mapped `columns`, and `errors`. Each row error includes `row`, `code`, `message`, and `fields`. A response containing accepted targets is not proof that every input row was accepted.

See [groups](../user-guide/groups.md) for encoding, limits, and duplicate handling. Save the reviewed targets with the [Groups API](groups.md).

## Email and website import

`POST /api/import/email` accepts `content` (raw MIME email as a JSON string) and optional `convert_links` (default false). It returns `subject`, `text`, and `html`. It does not import MIME attachments; upload supported attachments separately.

`POST /api/import/site` accepts `url` and returns `html`. Fetches follow outbound destination and TLS restrictions. The legacy `include_resources` input does not turn this into an asset-packaging service. Review imported HTML under the [landing-page restrictions](../user-guide/landing-pages.md).

## Preview HTML

Send `html` and optional `recipient` to `POST /api/util/preview`. Recipient fields are `email`, `first_name`, `last_name`, and `position`. The response contains rendered `html`:

```json
{
  "html": "<p>Hello {{.FirstName}}</p>",
  "recipient": {"email": "alex@example.test", "first_name": "Alex"}
}
```

A successful preview establishes rendering for those inputs. It does not verify mail-client layout, public routing, or launch eligibility.

For the restricted representation of a landing page, use [`POST /api/pages/preview`](landing-pages.md#preview-a-landing-page). For static learning content, use [`POST /api/learning-pages/preview`](learning-pages.md). Both page endpoints apply the same document preparation and sanitizer as public page serving; `/api/util/preview` retains its template/email-preview behavior.

## SMTP checks

`POST /api/util/test_connection` accepts a sending-profile object and returns a success envelope when connection verification succeeds. `POST /api/util/send_test_email` accepts `template`, `page`, `smtp`, `url`, and recipient fields `email`, `first_name`, `last_name`, and `position`. It returns a success envelope after accepted submission.

Test sends are external effects. Use a controlled inbox and inspect the received message before sending another. A failed or lost response can have an uncertain SMTP outcome; do not automatically repeat it.
