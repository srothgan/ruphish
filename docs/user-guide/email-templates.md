---
title: Create email templates
description: Write, import, personalize, and test HTML and plain-text campaign messages.
sidebar:
  order: 4
---

An email template contains your subject, HTML and/or plain-text body, optional visible sender override, and attachments. Start with a small message and check a personalized example before adding more content.

Administrators can also add an independent copy from the [Starter Template Repository](starter-templates.md).

## Write a message

Open **Resources → Email templates → Create**. Give the template a name, enter its subject, and write the body in the source or visual editor. At least one body alternative is required; HTML-only and text-only templates are supported. The subject may be empty.

Use `{{.FirstName}}` to personalize a greeting and `{{.URL}}` for the recipient's campaign link:

```html
<p>Hello {{.FirstName}},</p>
<p><a href="{{.URL}}">Open the training page</a>.</p>
```

When **Track email opens** is enabled for the campaign, Ruphish adds one hidden tracking image when composing HTML email. Turn it off during campaign setup to omit the image and prevent open observations. The choice defaults to enabled and is fixed at launch. You do not need to insert a tracker into the template, and previews omit it. Plain-text-only messages have no image-based open tracking. Mail clients can block image requests, so a missing open event is not proof the email was unread. Imported templates may still contain legacy `{{.Tracker}}` or `{{.TrackingURL}}` expressions. During sending, the composer removes images for the rendered recipient tracking URL, then inserts one managed image when tracking is enabled or none when disabled. Ordinary images are retained; you do not need a one-time template conversion.

Provide a plain-text alternative when it helps recipients whose clients do not display HTML. The [template reference](template-reference.md) covers variables, conditional content, and escaping.

## Import an email

Choose **Import** from the template list and provide a raw email message. Import extracts the subject and body alternatives. MIME attachments are not imported automatically; add them separately. Review the imported content before saving, especially converted campaign links and remote images.

The import draft is temporary until **Create** succeeds. Resource JSON imports preserve supported attachment content but discard imported identities and ownership. Reloading the tab loses an unsaved import draft.

## Sender and attachments

The template's visible **From** override is different from the SMTP envelope sender supplied by the sending profile. The legacy API calls the template override `envelope_sender`; despite that name, it is the visible sender field.

Use the [attachments guide](attachments.md) for supported personalization formats and limits. Check both message and attachment output with synthetic recipient details before launch.

## Preview and test

Preview rendering checks personalization without creating campaign events. The visual editor has its own Apply/Cancel actions; apply the visual changes before saving the resource.

Open **Send test**, then use **Send test email** to submit to one chosen recipient. If the template or profile has unsaved changes, **Save and send test** saves them first. A successful save remains saved if the send fails. Success means the SMTP server accepted the message, not that it reached an inbox. A connection loss after submission can leave the outcome unknown; inspect the receiving service before sending another copy.

Editing content referenced by an existing campaign can affect its consumers. Create a separate template when you need an independent version.
