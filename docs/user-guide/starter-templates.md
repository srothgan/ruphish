---
title: Add starter templates
description: Preview campaign content and add independent resource copies from the offline company catalog.
sidebar:
  order: 5
---

Starter templates provide a starting point for campaign content. An email template is the message a campaign sends to recipients. A landing page is the page recipients see after following the campaign link. A [learning page](learning-pages.md) provides a static explanation after submission.

The **Starter Template Repository** lets administrators preview the available designs and choose which ones to add to their resources. Each addition creates an independent copy that you can edit and use in campaigns. Emails, landing pages and learning pages are chosen separately; they are not fixed sets. You can return to add more whenever needed. The collection is included with Ruphish and does not require an internet download.

Open **Company settings → Starter Template Repository** at `/settings/starter-templates`. Supported landing pages offer recipient email prefill. Landing pages and learning pages can offer a custom company logo, which you can try in the preview before adding them.

1. Choose **Email templates**, **Landing pages** or **Learning pages** and search by name or brand. The table lists available options and lets you select entries directly. **Preview** opens a large dialog with the title and options above the viewer, **Previous** and **Next** below it, and an **X** in the top-right corner. Navigation follows the filtered table and stops at the first and last entries. Close with **X** or Escape to return to the table and its initiating Preview button.
2. Try **Prefill recipient email** or **Use company logo** where offered, before or after selecting a page. Recipient email prefill applies only to landing pages. Both default off. **Selected for import** in the dialog and the table checkbox control the same selection. Options remain when you navigate between previews, close the dialog, deselect the entry or switch tabs.
3. Click **Use company logo** above the **Landing pages** or **Learning pages** list to reveal the upload controls and image preview. **Choose image** accepts a local PNG, JPEG or WebP file, up to 5 MB and 4096 × 4096 pixels; no hosted URL is required. Use **Replace image** to change it or **Remove** to clear it. A failed replacement keeps your current image. Hiding the controls or switching tabs preserves the image. If a template has branding enabled without an image, **Open logo upload** closes its preview dialog and takes you to **Choose image**. The shared image applies only to templates where you enable branding. Preview prefill uses Alice Example; campaigns use their actual recipient.
4. Select the entries you want to add. Only selected pages with company branding enabled require an uploaded image before adding copies; configuring an unselected preview does not block other additions.
5. Choose **Add selected**. Review each result and follow its link to the ordinary resource page.

The catalog is read-only. After adding, edit, export or delete the copy through **Resources → Email templates**, **Resources → Landing pages** or **Resources → Learning pages**. Copies belong to the current administrator and have no catalog relationship, update checks or installed status. The logo is embedded in the copied HTML. Prefill becomes the existing `{{.Email | html}}` expression. Subsequent catalog changes cannot change your resources.

Return whenever needed. Adding the same entry again creates another resource, using a name such as `Microsoft 365 - Shared document (EN) (2)` when necessary. Existing work is never overwritten. Each creation is independent, so a batch can partially succeed. If a request is interrupted, check your resources before selecting and adding that entry again; the server may have saved a copy before the response was lost.

Learning-page previews show static content without recipient personalization, forms or tracking. For a page that offers **Use company logo**, enable it in the preview to embed the shared uploaded image in that copy. Learning pages do not offer recipient-email prefill. After adding a copy, select it as the redirect destination during campaign creation.

## Document invitations and interactive sign-in

The catalog contains 23 emails, 25 landing pages and three learning pages. The following options are additional entries; the original templates remain available.

| Landing page | Matching email | Sample document |
| --- | --- | --- |
| OneDrive / SharePoint - Shared document | OneDrive - Document sharing invitation (EN) | Alex Morgan shares `Project handover.docx`. |
| Google Drive - Shared document | Google Drive - Document sharing invitation (EN) | Jordan Lee shares `Project review notes`. |
| Dropbox - Shared file | Dropbox - File sharing invitation (EN) | Taylor Chen shares `Project brief.pdf`. |

These pages represent restricted sharing that asks the recipient to sign in with an account, rather than enter a document password. Add the email and landing page separately, then select both in the campaign. If you customize the sender or filename, edit both copies; they do not synchronize automatically.

Three standalone pages are also available: **Microsoft Entra - Two-step sign-in**, **Microsoft Entra - Two-step sign-in (side panel)** and **Google Workspace - Two-step sign-in**. They ask for an email address before showing the password field. The OneDrive and Google Drive document pages use the same sequence after opening the document card; Dropbox shows email and password together. Going back before final submission clears the password and preserves the email address.

Opening a document card, continuing after the email address and going back create no submission events. Only the final sign-in action calls the built-in collector, which records password presence without sending the entered values. Final submission locks the form against duplicate actions; uncertain delivery uses the collector's **Retry this submission** button. Functional previews create no campaign events. Close and reopen a preview to restart after final submission. These pages do not validate credentials or retrieve cloud files.

## Learning page choices

The **Learning pages** tab offers three independent English starters. None is preselected or preferred; choose the method and layout that suit your audience.

| Starter | Learning method | Layout |
| --- | --- | --- |
| SLAM | Inspect the Sender, Links, Attachments and Message, then verify uncertain requests independently. | A four-part inspection sheet with letter markers and examples alongside each check. |
| Stop, Challenge, Protect | Pause under pressure, question and independently verify the request, then report or seek help. | Three full-width stages, each pairing a decision question with practical actions. |
| Recognize, Resist and Report, Delete | Recognize warning signs, avoid engaging, report, then remove the message according to your organisation’s instructions. | A response guide with a side navigation and numbered action sections. |

All three use Ruphish styling and an embedded Ruphish logo. Enable **Use company logo** for an entry to replace its logo with your uploaded image; the lesson and layout stay intact. Each page includes illustrative examples and guidance for someone who already interacted with a suspicious message. Edit the imported copy to tailor its reporting instructions to your organisation.

## Company logo availability

| Page | Reviewed branding context |
| --- | --- |
| Microsoft full-screen | Entra company branding |
| Microsoft side panel | Entra company branding |
| Microsoft Entra - Two-step sign-in | Entra company branding |
| Microsoft Entra - Two-step sign-in (side panel) | Entra company branding |
| OneDrive / SharePoint - Shared document | Entra company branding on the sign-in panel |
| Zoom | Organization vanity-URL login |
| Salesforce | My Domain login |
| Workday | Native tenant login |
| SLAM | Learning page header |
| Stop, Challenge, Protect | Learning page header |
| Recognize, Resist and Report, Delete | Learning page header |

Other landing pages retain their vendor branding. These options describe the bundled layouts, not universal capabilities of every vendor login screen. No uploaded image is linked externally or shared with resources that have already been created.

All content and logos are bundled offline. Previews make no tracking requests. [Email tracking](email-templates.md) is added during sending, and [landing-page privacy](landing-pages.md#forms-and-submission-tracking) applies to catalog copies just as it does to any other page. For automation, see the [catalog API](../api/starter-templates.md).
