---
title: Starter templates
description: Read-only catalog metadata and resource-creation payloads.
sidebar:
  order: 42
---

The bundled starter catalog requires a Bearer API key with `modify_system`. It is read-only; copying a catalog entry does not create a resource until you POST its resource payload to the appropriate collection.

## GET `/api/starter-templates/`

List the bundled catalog metadata.

**Request:** No body. **Response:** `200 OK`, `Vec<starter_templates::Summary>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/starter-templates/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

<details>
<summary>Example JSON response (complete generated content)</summary>

```json
[
  {
    "id": "email-onedrive-document-invitation",
    "name": "OneDrive - Document sharing invitation (EN)",
    "brand": "Microsoft",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-google-drive-document-invitation",
    "name": "Google Drive - Document sharing invitation (EN)",
    "brand": "Google",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-dropbox-file-invitation",
    "name": "Dropbox - File sharing invitation (EN)",
    "brand": "Dropbox",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "page-onedrive-shared-document",
    "name": "OneDrive / SharePoint - Shared document",
    "brand": "Microsoft",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".microsoft",
      "height": 36,
      "context": "Entra company branding"
    }
  },
  {
    "id": "page-google-drive-shared-document",
    "name": "Google Drive - Shared document",
    "brand": "Google",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-dropbox-shared-file",
    "name": "Dropbox - Shared file",
    "brand": "Dropbox",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-microsoft-two-step",
    "name": "Microsoft Entra - Two-step sign-in",
    "brand": "Microsoft",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".microsoft",
      "height": 36,
      "context": "Entra company branding"
    }
  },
  {
    "id": "page-microsoft-two-step-side-panel",
    "name": "Microsoft Entra - Two-step sign-in (side panel)",
    "brand": "Microsoft",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".microsoft",
      "height": 36,
      "context": "Entra company branding"
    }
  },
  {
    "id": "page-google-workspace-two-step",
    "name": "Google Workspace - Two-step sign-in",
    "brand": "Google",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "email-microsoft-shared-document",
    "name": "Microsoft 365 - Shared document (EN)",
    "brand": "Microsoft",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-microsoft-account-security",
    "name": "Microsoft account - Account security (EN)",
    "brand": "Microsoft",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-microsoft-teams-invitation",
    "name": "Microsoft Teams - Team invitation (EN)",
    "brand": "Microsoft",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-google-shared-document",
    "name": "Google Workspace - Shared document (EN)",
    "brand": "Google",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-google-account-security",
    "name": "Google Account - Account security (EN)",
    "brand": "Google",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-dropbox-shared-folder",
    "name": "Dropbox - Shared folder (EN)",
    "brand": "Dropbox",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-docusign-signature-request",
    "name": "Docusign - Signature request (EN)",
    "brand": "Docusign",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-adobe-document-review",
    "name": "Adobe Acrobat - PDF review (EN)",
    "brand": "Adobe",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-slack-workspace-invitation",
    "name": "Slack - Workspace invitation (EN)",
    "brand": "Slack",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-zoom-meeting-invitation",
    "name": "Zoom - Meeting invitation (EN)",
    "brand": "Zoom",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-atlassian-jira-assignment",
    "name": "Jira - Task assignment (EN)",
    "brand": "Atlassian",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-atlassian-confluence-access",
    "name": "Confluence - Knowledge-space access (EN)",
    "brand": "Atlassian",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-salesforce-report-access",
    "name": "Salesforce - Shared report (EN)",
    "brand": "Salesforce",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-github-repository-invitation",
    "name": "GitHub - Repository invitation (EN)",
    "brand": "GitHub",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-github-account-security",
    "name": "GitHub - Account security (EN)",
    "brand": "GitHub",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-paypal-payment-request",
    "name": "PayPal - Payment request (EN)",
    "brand": "PayPal",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-amazon-business-invoice",
    "name": "Amazon Business - Invoice available (EN)",
    "brand": "Amazon",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-apple-icloud-storage",
    "name": "iCloud - Storage review (EN)",
    "brand": "Apple",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-linkedin-message-notification",
    "name": "LinkedIn - Message notification (EN)",
    "brand": "LinkedIn",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "email-workday-employee-document",
    "name": "Workday - Employee document (EN)",
    "brand": "Workday",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  {
    "id": "page-microsoft-full-screen",
    "name": "Microsoft - Sign in (full-screen background)",
    "brand": "Microsoft",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".microsoft",
      "height": 36,
      "context": "Entra company branding"
    }
  },
  {
    "id": "page-microsoft-side-panel",
    "name": "Microsoft - Sign in (side panel)",
    "brand": "Microsoft",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".microsoft",
      "height": 36,
      "context": "Entra company branding"
    }
  },
  {
    "id": "page-google-workspace",
    "name": "Google - Sign in (wide)",
    "brand": "Google",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-google-account",
    "name": "Google - Sign in (compact)",
    "brand": "Google",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-dropbox",
    "name": "Dropbox - Log in",
    "brand": "Dropbox",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-docusign",
    "name": "Docusign - Log in",
    "brand": "Docusign",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-adobe-acrobat",
    "name": "Adobe - Sign in",
    "brand": "Adobe",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-slack",
    "name": "Slack - Password sign-in adaptation",
    "brand": "Slack",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-zoom",
    "name": "Zoom - Sign in",
    "brand": "Zoom",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".masthead",
      "height": 36,
      "context": "Organization vanity-URL login"
    }
  },
  {
    "id": "page-jira",
    "name": "Atlassian Cloud - Log in",
    "brand": "Atlassian",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-confluence",
    "name": "Confluence Cloud - Atlassian sign-in",
    "brand": "Atlassian",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-salesforce",
    "name": "Salesforce - Log in",
    "brand": "Salesforce",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": "img.logo",
      "height": 80,
      "context": "My Domain login"
    }
  },
  {
    "id": "page-github-repository",
    "name": "GitHub - Sign in (light)",
    "brand": "GitHub",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-github-account",
    "name": "GitHub - Sign in (dark)",
    "brand": "GitHub",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-paypal",
    "name": "PayPal - Log in",
    "brand": "PayPal",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-amazon-business",
    "name": "Amazon Business - Sign in",
    "brand": "Amazon",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-apple-icloud",
    "name": "Apple Account - iCloud sign-in",
    "brand": "Apple",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-linkedin",
    "name": "LinkedIn - Sign in",
    "brand": "LinkedIn",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": null
  },
  {
    "id": "page-workday",
    "name": "Workday - Native tenant sign-in",
    "brand": "Workday",
    "kind": "landing_page",
    "prefill": true,
    "company_logo": {
      "selector": ".brand",
      "height": 48,
      "context": "Native tenant login"
    }
  },
  {
    "id": "learning-slam",
    "name": "SLAM (EN)",
    "brand": "Ruphish",
    "kind": "learning_page",
    "prefill": false,
    "company_logo": {
      "selector": ".learning-logo",
      "height": 48,
      "context": "Learning page header"
    }
  },
  {
    "id": "learning-stop-challenge-protect",
    "name": "Stop, Challenge, Protect (EN)",
    "brand": "Ruphish",
    "kind": "learning_page",
    "prefill": false,
    "company_logo": {
      "selector": ".learning-logo",
      "height": 48,
      "context": "Learning page header"
    }
  },
  {
    "id": "learning-recognize-resist-report-delete",
    "name": "Recognize, Resist and Report, Delete (EN)",
    "brand": "Ruphish",
    "kind": "learning_page",
    "prefill": false,
    "company_logo": {
      "selector": ".learning-logo",
      "height": 48,
      "context": "Learning page header"
    }
  }
]
```

</details>

### Catalog summary fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `string` | Catalog lookup key, never a persistent resource relationship. |
| `name` | `string` | Suggested ordinary resource name. |
| `brand` | `string` | Searchable brand name. |
| `kind` | `Kind` | Creation API family. |
| `prefill` | `boolean` | Whether recipient email prefill is supported; selection defaults off. |
| `company_logo` | `CompanyLogo or null` | Available only for reviewed branding variants; selection defaults off. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `selector` | `string` | Unique selector in this catalog entry, never stored on the created resource. |
| `height` | `integer` | Maximum displayed image height in CSS pixels. |
| `context` | `string` | Page or vendor configuration represented by the branded variant. |

`kind` is `email`, `landing_page`, or `learning_page`. The catalog ID is a string lookup key; it is not a saved numeric resource ID. `prefill` and `company_logo` describe capabilities of the catalog content, not writable resource fields.

## GET `/api/starter-templates/{catalog_id}`

Read a catalog entry and its ordinary resource-creation payload.

**Request:** No body. **Response:** `200 OK`, `starter_templates::Entry` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/starter-templates/email-onedrive-document-invitation" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "summary": {
    "id": "email-onedrive-document-invitation",
    "name": "OneDrive - Document sharing invitation (EN)",
    "brand": "Microsoft",
    "kind": "email",
    "prefill": false,
    "company_logo": null
  },
  "resource": {
    "name": "OneDrive - Document sharing invitation (EN)",
    "html": "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>OneDrive: Project handover.docx</title></head>\n<body style=\"margin:0;background:#f5f5f5;font-family:'Segoe UI',Arial,sans-serif;color:#242424;\">\n<div style=\"display:none;font-size:1px;max-height:0;overflow:hidden;\">Alex Morgan invited you to view Project handover.docx.</div>\n<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\"><tr><td align=\"center\" style=\"padding:32px 12px;\">\n<table role=\"presentation\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:100%;max-width:600px;background:#fff;border:1px solid #e0e0e0;\">\n<tr><td style=\"padding:28px 32px;border-bottom:1px solid #e0e0e0;\"><img src=\"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAGAAAABgCAYAAADimHc4AAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7DAcdvqGQAAAENSURBVHhe7dyxCcJgFEbRPw4guIITOYC7uIwD2OowriC4QGwlhWCTS+Cc8nWP23/T+3Scx4bsb89peft2eYxN/bNbHliXADEBYgLEBIgJEBMgJkBMgJgAMQFiAsQEiAkQEyAmQEyAmAAxAWICxASICRATICZATICYADEBYgLEBIgJEBMgJkBMgJgAMQFiAsQEiAkQEyAmQEyAmAAxAQAAAFY2jetrUxtr43z4uRk3323G8QcBYgLEBIgJEBMgJkBMgJgAMQFiAsQEiAkQEyAmQEyAmAAxAWICxASICRATICZATICYADEBYgLEBIgJEBMgJkBMgJgAMQFiAsQEiAkQEyAmQEyAmAAxAWICxD6l3AyeLyC7VgAAAABJRU5ErkJggg==\" alt=\"Microsoft\" width=\"28\" height=\"28\" style=\"vertical-align:middle;\"> <span style=\"font-size:22px;vertical-align:middle;\">OneDrive</span></td></tr>\n<tr><td style=\"padding:32px;\"><h1 style=\"margin:0 0 24px;font-size:24px;line-height:1.4;font-weight:500;\">Alex Morgan shared a document with you</h1>\n<p style=\"font-size:15px;line-height:1.6;margin:0 0 20px;\">Hello {{if .FirstName}}{{.FirstName | html}}{{else}}there{{end}},</p>\n<p style=\"font-size:15px;line-height:1.6;\">You have been invited to view the following document.</p>\n<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"border:1px solid #dadce0;\"><tr><td width=\"60\" style=\"padding:20px 0 20px 20px;color:#0078d4;font-size:12px;font-weight:bold;\">DOCX</td><td style=\"padding:20px;\"><p style=\"margin:0 0 5px;font-size:17px;font-weight:600;\">Project handover.docx</p><p style=\"margin:0;font-size:13px;color:#666;\">Word document &middot; Can view</p></td></tr></table>\n<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin:28px 0;\"><tr><td bgcolor=\"#0078d4\" style=\"background:#0078d4;border-radius:0;\"><a href=\"{{.URL | html}}\" style=\"display:inline-block;padding:12px 28px;color:#fff;font-size:15px;font-weight:600;text-decoration:none;\">Open document</a></td></tr></table>\n<p style=\"font-size:13px;line-height:1.6;color:#666;\">This invitation is for {{.Email | html}}. Sign in with that account to access the document.</p></td></tr>\n<tr><td style=\"padding:20px 32px;border-top:1px solid #e0e0e0;font-size:12px;color:#666;\">OneDrive &middot; Shared by Alex Morgan</td></tr></table></td></tr></table>\n</body></html>\n",
    "subject": "Alex Morgan shared \"Project handover.docx\" with you",
    "text": "Alex Morgan shared a file with you\n\nProject handover.docx\nWord document - Can view\n\nOpen the file: {{.URL}}\n\nThis invitation is for {{.Email}}. Sign in with that account to access the file.\n\nOneDrive\n",
    "attachments": []
  }
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `summary` | `Summary` | Catalog-only information. |
| `resource` | `serde_json::Value` | An ordinary create payload; contains no catalog identifier or options. |

Submit only `resource`, after any edits, to `/api/templates/` for email, `/api/pages/` for a landing page, or `/api/learning-pages/` for a learning page. The resulting resource is an independent copy. An unknown catalog ID returns `404 RESOURCE_NOT_FOUND`.
