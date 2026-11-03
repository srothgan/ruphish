---
title: API migration
description: Map each Gophish endpoint to its Ruphish contract and required client changes.
sidebar:
  order: 60
prev: {"label": "Migration overview", "link": "/docs/migration/"}
next: {"label": "Common contract changes", "link": "/docs/migration/api/common-contract-changes/"}
---

Migrate the database using the [shared procedure](../migrate-the-database.md), and adapt each operation your client uses before allowing production writes. This reference compares Gophish Community Edition 0.12.1, commit `95618469799295e2c0fec980805a2dfbb818816b`, with the current Ruphish contracts introduced with 0.1.0.

Start with [Common contract changes](common-contract-changes.md). Every Gophish operation below has its own comparison page with requests, responses, attribute differences, and behavior. An unchanged path is not a compatibility guarantee. New capabilities link to the current HTTP API rather than repeating its reference.

## Endpoint inventory

The inventory covers 49 intended management operations in the pinned Gophish router/handlers. Slash redirects and accidental acceptance of unsupported methods are not additional client operations. All Ruphish integration examples use Bearer API keys.

### Groups

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/groups/` | `GET /api/groups/` | Contract or behavior changed | [List groups](groups/list-groups.md) |
| `GET /api/groups/summary` | `GET /api/groups/summary` | Contract or behavior changed | [List group summaries](groups/list-group-summaries.md) |
| `GET /api/groups/{id}` | `GET /api/groups/{id}` | Contract or behavior changed | [Read a group](groups/read-a-group.md) |
| `GET /api/groups/{id}/summary` | `GET /api/groups/{id}/summary` | Contract or behavior changed | [Read a group summary](groups/read-a-group-summary.md) |
| `POST /api/groups/` | `POST /api/groups/` | Contract or behavior changed | [Create a group](groups/create-a-group.md) |
| `PUT /api/groups/{id}` | `PUT /api/groups/{id}` | Contract or behavior changed | [Update a group](groups/update-a-group.md) |
| `DELETE /api/groups/{id}` | `DELETE /api/groups/{id}` | Contract or behavior changed | [Delete a group](groups/delete-a-group.md) |

### Email templates

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/templates/` | `GET /api/templates/` | Contract or behavior changed | [List email templates](email-templates/list-email-templates.md) |
| `GET /api/templates/{id}` | `GET /api/templates/{id}` | Contract or behavior changed | [Read an email template](email-templates/read-an-email-template.md) |
| `POST /api/templates/` | `POST /api/templates/` | Contract or behavior changed | [Create an email template](email-templates/create-an-email-template.md) |
| `PUT /api/templates/{id}` | `PUT /api/templates/{id}` | Contract or behavior changed | [Update an email template](email-templates/update-an-email-template.md) |
| `DELETE /api/templates/{id}` | `DELETE /api/templates/{id}` | Contract or behavior changed | [Delete an email template](email-templates/delete-an-email-template.md) |

### Landing pages

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/pages/` | `GET /api/pages/` | Contract or behavior changed | [List landing pages](landing-pages/list-landing-pages.md) |
| `GET /api/pages/{id}` | `GET /api/pages/{id}` | Contract or behavior changed | [Read a landing page](landing-pages/read-a-landing-page.md) |
| `POST /api/pages/` | `POST /api/pages/` | Contract or behavior changed | [Create a landing page](landing-pages/create-a-landing-page.md) |
| `PUT /api/pages/{id}` | `PUT /api/pages/{id}` | Contract or behavior changed | [Update a landing page](landing-pages/update-a-landing-page.md) |
| `DELETE /api/pages/{id}` | `DELETE /api/pages/{id}` | Contract or behavior changed | [Delete a landing page](landing-pages/delete-a-landing-page.md) |

### Sending profiles

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/smtp/` | `GET /api/smtp/` | Contract or behavior changed | [List sending profiles](sending-profiles/list-sending-profiles.md) |
| `GET /api/smtp/{id}` | `GET /api/smtp/{id}` | Contract or behavior changed | [Read a sending profile](sending-profiles/read-a-sending-profile.md) |
| `POST /api/smtp/` | `POST /api/smtp/` | Contract or behavior changed | [Create a sending profile](sending-profiles/create-a-sending-profile.md) |
| `PUT /api/smtp/{id}` | `PUT /api/smtp/{id}` | Contract or behavior changed | [Update a sending profile](sending-profiles/update-a-sending-profile.md) |
| `DELETE /api/smtp/{id}` | `DELETE /api/smtp/{id}` | Contract or behavior changed | [Delete a sending profile](sending-profiles/delete-a-sending-profile.md) |

### Imports

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `POST /api/import/group` | `POST /api/import/group` | Contract or behavior changed | [Import recipient CSV](imports/import-recipient-csv.md) |
| `POST /api/import/email` | `POST /api/import/email` | Contract or behavior changed | [Import email content](imports/import-email-content.md) |
| `POST /api/import/site` | `POST /api/import/site` | Contract or behavior changed | [Import a website](imports/import-a-website.md) |

### Test sends

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `POST /api/util/send_test_email` | `POST /api/util/send_test_email` | Contract or behavior changed | [Send a test email](send-a-test-email.md) |

### Campaigns and results

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `POST /api/campaigns/` | `POST /api/campaigns/` | Contract or behavior changed | [Create a campaign](campaigns/create-a-campaign.md) |
| `GET /api/campaigns/` | `GET /api/campaigns/` | Contract or behavior changed | [List campaigns](campaigns/list-campaigns.md) |
| `GET /api/campaigns/summary` | `GET /api/campaigns/summary` | Contract or behavior changed | [List campaign summaries](campaigns/list-campaign-summaries.md) |
| `GET /api/campaigns/{id}` | `GET /api/campaigns/{id}` | Contract or behavior changed | [Read a campaign](campaigns/read-a-campaign.md) |
| `GET /api/campaigns/{id}/summary` | `GET /api/campaigns/{id}/summary` | Contract or behavior changed | [Read a campaign summary](campaigns/read-a-campaign-summary.md) |
| `GET /api/campaigns/{id}/results` | `GET /api/campaigns/{id}/results` | Contract or behavior changed | [Read campaign results](campaigns/read-campaign-results.md) |
| `GET /api/campaigns/{id}/complete` | `POST /api/campaigns/{id}/complete` | Method changed | [Complete a campaign](campaigns/complete-a-campaign.md) |
| `DELETE /api/campaigns/{id}` | `DELETE /api/campaigns/{id}` | Contract or behavior changed | [Delete a campaign](campaigns/delete-a-campaign.md) |

### Reporting mailboxes

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/imap/` | `GET /api/reporting-mailboxes/` | Replaced | [Replace the IMAP read](reporting-mailboxes/replace-the-imap-read.md) |
| `POST /api/imap/` | `PUT /api/reporting-mailboxes/{id}` | Replaced | [Replace the IMAP save](reporting-mailboxes/replace-the-imap-save.md) |
| `POST /api/imap/validate` | `POST /api/reporting-mailboxes/{id}/validate` | Replaced | [Replace IMAP validation](reporting-mailboxes/replace-imap-validation.md) |

### Webhooks

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/webhooks/` | `GET /api/webhooks/` | Contract or behavior changed | [List webhooks](webhooks/list-webhooks.md) |
| `GET /api/webhooks/{id}` | `GET /api/webhooks/{id}` | Contract or behavior changed | [Read a webhook](webhooks/read-a-webhook.md) |
| `POST /api/webhooks/` | `POST /api/webhooks/` | Contract or behavior changed | [Create a webhook](webhooks/create-a-webhook.md) |
| `PUT /api/webhooks/{id}` | `PUT /api/webhooks/{id}` | Contract or behavior changed | [Update a webhook](webhooks/update-a-webhook.md) |
| `POST /api/webhooks/{id}/validate` | `POST /api/webhooks/{id}/validate` | Contract or behavior changed | [Validate a webhook](webhooks/validate-a-webhook.md) |
| `DELETE /api/webhooks/{id}` | `DELETE /api/webhooks/{id}` | Contract or behavior changed | [Delete a webhook](webhooks/delete-a-webhook.md) |

### Accounts and keys

| Gophish operation | Ruphish operation | Disposition | Comparison |
| --- | --- | --- | --- |
| `GET /api/users/` | `GET /api/users/` | Contract or behavior changed | [List accounts](accounts/list-accounts.md) |
| `GET /api/users/{id}` | `GET /api/users/{id}` | Contract or behavior changed | [Read an account](accounts/read-an-account.md) |
| `POST /api/users/` | `POST /api/users/` | Contract or behavior changed | [Create an account](accounts/create-an-account.md) |
| `PUT /api/users/{id}` | `PUT /api/users/{id}` | Contract or behavior changed | [Update an account](accounts/update-an-account.md) |
| `DELETE /api/users/{id}` | `DELETE /api/users/{id}` | Contract or behavior changed | [Delete an account](accounts/delete-an-account.md) |
| `POST /api/reset` | `POST /api/reset` | Contract or behavior changed | [Rotate your API key](accounts/rotate-your-api-key.md) |

## New operations

| Capability | Current reference |
| --- | --- |
| Prepare campaign selections, review deduplication, send a campaign sample, duplicate configuration, or use paged directory/overview reads | [Campaigns](../../api/campaigns.md) |
| Save, update, delete, and launch campaign drafts | [Campaign drafts](../../api/campaign-drafts.md) |
| Inspect delivery attempts and reconcile unknown or failed deliveries | [Delivery and history](../../api/delivery-and-history.md) |
| Add, edit, or delete an operator event observation | [Event corrections](../../api/event-corrections.md) |
| Filter analysis and download CSV/XLSX | [Results and exports](../../api/results-and-exports.md) |
| Register and verify campaign origins | [Domains](../../api/domains.md) |
| Manage learning pages | [Learning pages](../../api/learning-pages.md) |
| Create/delete extra mailboxes, schedule checks, and resume paused processing | [Reporting mailboxes](../../api/reporting-mailboxes.md) |
| Preview/validate content, review CSV, import ZIP page bundles, or test an SMTP connection | [Imports, previews, and test sends](../../api/imports-previews-and-test-sends.md) |
| Read template/page/SMTP summaries | [Email templates](../../api/email-templates.md), [Landing pages](../../api/landing-pages.md), [Sending profiles](../../api/sending-profiles.md) |
| Read dashboard aggregates | [Overview](../../api/overview.md) |
| Set company policy and close active campaigns in bulk | [Company settings](../../api/company-settings.md) |
| Browse starter content | [Starter templates](../../api/starter-templates.md) |

Webhook receivers also need the separate [delivery comparison](webhook-delivery-changes.md): event payloads, acknowledgements, and retries are part of the integration contract even though they are outbound requests.

## Integration acceptance

Test each used operation against a migrated rehearsal copy. Include a representative ordinary account, a stale write, missing historical fields, multiple result pages, and failed network actions. Verify behavior and side effects as well as JSON parsing. Complete [Verification and rollback](../verification-and-rollback.md) before cutover.
