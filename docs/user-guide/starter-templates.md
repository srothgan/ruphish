---
title: Add starter templates
description: Select independent email and landing-page copies from the offline company catalog.
sidebar:
  order: 5
---

Administrators can open **Company settings → Starter Template Repository** at `/settings/starter-templates`. The bundled collection contains 20 English email templates and 19 landing pages. Emails and landing pages are separate selections; adding one never adds or pairs it with another.

1. Choose **Email templates** or **Landing pages**, search by name or brand, and open a preview.
2. Select the entries you want. Selections remain available when switching tabs.
3. For a selected landing page, optionally enable **Prefill recipient email** or **Use company logo** where offered. Both default off.
4. If any selected page uses company branding, upload one shared PNG, JPEG or WebP image, up to 512 KB and 4096 × 4096 pixels. The preview reflects the chosen logo and prefill setting. The sample recipient is Alice Example; campaigns use their actual recipient.
5. Choose **Add selected**. Review each result and follow its link to the ordinary resource page.

The catalog is read-only. After adding, edit, export or delete the copy through **Resources → Email templates** or **Resources → Landing pages**. Copies belong to the current administrator and have no catalog relationship, update checks or installed status. The logo is embedded in the copied HTML. Prefill becomes the existing `{{.Email | html}}` expression. Subsequent catalog changes cannot change your resources.

Return whenever needed. Adding the same entry again creates another resource, using a name such as `Microsoft 365 - Shared document (EN) (2)` when necessary. Existing work is never overwritten. Each creation is independent, so a batch can partially succeed. If a request is interrupted, check your resources before selecting and adding that entry again; the server may have saved a copy before the response was lost.

## Company logo availability

| Landing page | Reviewed branding context |
| --- | --- |
| Microsoft full-screen | Entra company branding |
| Microsoft side panel | Entra company branding |
| Zoom | Organization vanity-URL login |
| Salesforce | My Domain login |
| Workday | Native tenant login |

Other entries retain their vendor branding. These options describe the bundled layouts, not universal capabilities of every vendor login screen. No uploaded image is linked externally or shared with resources that have already been created.

All content and logos are bundled offline. Previews make no tracking requests. [Email tracking](email-templates.md) is added during sending, and [landing-page privacy](landing-pages.md#forms-and-submission-tracking) applies to catalog copies just as it does to any other page. For automation, see the [catalog API](../api/starter-templates.md).
