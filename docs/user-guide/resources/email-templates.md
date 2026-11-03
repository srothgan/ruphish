---
title: Email templates
description: Write, import, personalize, and test campaign emails and attachments.
sidebar:
  order: 13
---

An email template contains the subject, message body, optional visible sender, and attachments recipients receive. You can reuse it across campaigns; each recipient gets their own personalized message and campaign link.

Start from [starter content](../starter-templates.md), import an existing email, or write a new message.

## Create a template

1. Open **Resources → Email templates** and choose **Create**.
2. Enter a name for the template and the email subject.
3. Write the HTML body using the source or visual editor. Apply your changes in the visual editor before saving.
4. Add a **Plain text alternative** if you want a version for clients that do not display HTML.
5. Include `{{.URL}}` where recipients should open the campaign page.
6. Choose **Create**.

You can use HTML, plain text, or both. At least one body is required. The template name helps you find it in Ruphish; the subject is what recipients see.

For example, this HTML personalizes the greeting and adds the campaign link:

```html
<p>Hello {{if .FirstName}}{{.FirstName | html}}{{else}}there{{end}},</p>
<p><a href="{{.URL | html}}">Open the shared document</a>.</p>
```

Use [Personalize content](../template-reference.md) for other fields, conditional text, and escaping. Do not replace `{{.URL}}` with a single recipient's copied campaign link.

## Import an email

From the template list, choose **Import** and select **Email (.eml)** to load an existing raw email. Review the subject, HTML, and plain-text body before creating the resource.

The import can replace links with the recipient's campaign URL. Review those links carefully: not every link in an imported message should necessarily lead to the landing page.

Email import does not automatically bring in MIME attachments. Upload them separately. **Resource JSON** imports can retain attachments exported with a Ruphish template.

## Set the visible sender

Leave **Visible From** empty to use the sending profile's sender address. Set an override when the message needs a different visible sender.

This changes the From header recipients see; it does not replace the account used to connect to SMTP. Check your provider's allowed senders and inspect both sender values during campaign review.

## Add attachments

Choose **Add attachments** in the template editor, select the files, and save the template. Use **Remove** beside an attachment to take it out.

Some file formats support the same personalization expressions as the message:

| File extensions | Personalization |
| --- | --- |
| `.txt`, `.html`, `.ics` | Expressions in UTF-8 text. |
| `.docx`, `.docm`, `.pptx`, `.xlsx`, `.xlsm` | Expressions in supported XML and relationship entries within the document. |
| Other extensions | Sent with their original contents. |

Use lowercase extensions for these formats. Office editors can split a visible expression across internal text fragments, so always open the attachment received in a test message and check the actual result.

Attachments have a 16 MiB decoded-content limit. The complete resource request also has a 32 MiB limit, including encoded attachment data. Large or unusually complex Office archives can fail additional processing limits; simplify the document or reduce its size.

Attachment opens are not a separate reliable measurement. A linked tracking image may be fetched by a document viewer, but that does not prove a person opened the attachment or enabled a document feature.

## Preview and send a test

Use **Preview** to check the rendered layout. Then choose **Send test**, select a [sending profile](sending-profiles.md), enter your own recipient details, and send the message to an inbox you control.

Check the subject, sender, personalized text, links, plain-text alternative, and attachments. Preview does not send email or create campaign activity.

For HTML messages, campaign open tracking is controlled by **Track email opens** when creating the campaign. Ruphish adds the tracking image automatically when enabled; you do not need to insert one. Plain-text messages cannot use image-based open tracking.

## Reuse or change a template

Edit the saved template for future campaigns, or choose **Duplicate** to create a separate variation. Launched campaigns keep their saved message content, even if sending is scheduled for later or retried.

Deletion is blocked while an unfinished campaign references the template.
