---
title: HTTP API
description: API-key reference for applications integrating with Ruphish.
sidebar:
  order: 0
---

Use Ruphish as a backend for your own application, user interface, or automation. This reference covers API-key access to resource management, campaign execution, delivery, results, and administration. It describes the current endpoint and JSON contracts.

Every documented request requires an API key:

```http
Authorization: Bearer YOUR_API_KEY
```

[Authentication](authentication.md) defines credentials and permissions. [Conventions](conventions.md) covers request formats, revisions, and pagination; [errors](errors.md) defines failure responses. Each endpoint page includes its purpose, request and response types, JSON examples, and field definitions.

## Endpoint index

Paths below use `{id}` for numeric resource IDs. Request examples use concrete IDs; replace them with IDs returned by your installation. Optional query parameters are documented on each page.

### Resources

| Method | Endpoint | Purpose | Reference |
| --- | --- | --- | --- |
| POST | `/api/groups/` | Create a resource. | [Groups](groups.md) |
| GET | `/api/groups/` | List the stored resources. The response is an array, including `[]` when empty. | [Groups](groups.md) |
| GET | `/api/groups/{id}` | Read one resource. | [Groups](groups.md) |
| PUT | `/api/groups/{id}` | Replace the editable content at the current revision. | [Groups](groups.md) |
| GET | `/api/groups/summary` | List resource metadata without authored content. | [Groups](groups.md) |
| GET | `/api/groups/{id}/summary` | Read metadata and recipient count for one group. | [Groups](groups.md) |
| DELETE | `/api/groups/{id}` | Delete the resource at the specified revision. | [Groups](groups.md) |
| POST | `/api/templates/` | Create a resource. | [Email templates](email-templates.md) |
| GET | `/api/templates/` | List the stored resources. The response is an array, including `[]` when empty. | [Email templates](email-templates.md) |
| GET | `/api/templates/{id}` | Read one resource. | [Email templates](email-templates.md) |
| PUT | `/api/templates/{id}` | Replace the editable content at the current revision. | [Email templates](email-templates.md) |
| GET | `/api/templates/summary` | List resource metadata without authored content. | [Email templates](email-templates.md) |
| DELETE | `/api/templates/{id}` | Delete the resource at the specified revision. | [Email templates](email-templates.md) |
| POST | `/api/pages/` | Create a resource. | [Landing pages](landing-pages.md) |
| GET | `/api/pages/` | List the stored resources. The response is an array, including `[]` when empty. | [Landing pages](landing-pages.md) |
| GET | `/api/pages/{id}` | Read one resource. | [Landing pages](landing-pages.md) |
| PUT | `/api/pages/{id}` | Replace the editable content at the current revision. | [Landing pages](landing-pages.md) |
| GET | `/api/pages/summary` | List resource metadata without authored content. | [Landing pages](landing-pages.md) |
| DELETE | `/api/pages/{id}` | Delete the resource at the specified revision. | [Landing pages](landing-pages.md) |
| POST | `/api/learning-pages/` | Create a resource. | [Learning pages](learning-pages.md) |
| GET | `/api/learning-pages/` | List the stored resources. The response is an array, including `[]` when empty. | [Learning pages](learning-pages.md) |
| GET | `/api/learning-pages/{id}` | Read one resource. | [Learning pages](learning-pages.md) |
| PUT | `/api/learning-pages/{id}` | Replace the editable content at the current revision. | [Learning pages](learning-pages.md) |
| GET | `/api/learning-pages/summary` | List resource metadata without authored content. | [Learning pages](learning-pages.md) |
| DELETE | `/api/learning-pages/{id}` | Delete the resource at the specified revision. | [Learning pages](learning-pages.md) |
| POST | `/api/smtp/` | Create a resource. | [Sending profiles](sending-profiles.md) |
| GET | `/api/smtp/` | List the stored resources. The response is an array, including `[]` when empty. | [Sending profiles](sending-profiles.md) |
| GET | `/api/smtp/{id}` | Read one resource. | [Sending profiles](sending-profiles.md) |
| PUT | `/api/smtp/{id}` | Replace the editable content at the current revision. | [Sending profiles](sending-profiles.md) |
| GET | `/api/smtp/summary` | List resource metadata without authored content. | [Sending profiles](sending-profiles.md) |
| DELETE | `/api/smtp/{id}` | Delete the resource at the specified revision. | [Sending profiles](sending-profiles.md) |
| POST | `/api/domains/` | Register an origin. This example requires domain verification to be disabled. | [Domains](domains.md) |
| GET | `/api/domains/` | List registered origins. | [Domains](domains.md) |
| GET | `/api/domains/validated/` | List origins that are eligible under the current policy, including origins that do not require verification. | [Domains](domains.md) |
| GET | `/api/domains/{id}` | Read the current policy result and verification history. | [Domains](domains.md) |
| PUT | `/api/domains/{id}` | Update the name or enablement at the supplied body revision. | [Domains](domains.md) |
| POST | `/api/domains/{id}/verify` | Queue verification for an HTTPS DNS origin. | [Domains](domains.md) |
| DELETE | `/api/domains/{id}` | Delete an unreferenced domain at its current revision. | [Domains](domains.md) |
| POST | `/api/reporting-mailboxes/` | Create the configuration. | [Reporting mailboxes](reporting-mailboxes.md) |
| GET | `/api/reporting-mailboxes/` | List configurations. | [Reporting mailboxes](reporting-mailboxes.md) |
| GET | `/api/reporting-mailboxes/{id}` | Read configuration and recent operational history. | [Reporting mailboxes](reporting-mailboxes.md) |
| PUT | `/api/reporting-mailboxes/{id}` | Replace configuration while preserving the saved secret. | [Reporting mailboxes](reporting-mailboxes.md) |
| POST | `/api/reporting-mailboxes/{id}/validate` | Validate the saved mailbox connection and credentials. | [Reporting mailboxes](reporting-mailboxes.md) |
| POST | `/api/reporting-mailboxes/{id}/check` | Schedule processing of the saved mailbox. | [Reporting mailboxes](reporting-mailboxes.md) |
| POST | `/api/reporting-mailboxes/{id}/resume` | Resume a mailbox paused by a UIDVALIDITY change at the expected revision. | [Reporting mailboxes](reporting-mailboxes.md) |
| DELETE | `/api/reporting-mailboxes/{id}` | Delete the configuration at its current revision. | [Reporting mailboxes](reporting-mailboxes.md) |
| POST | `/api/webhooks/` | Create the configuration. | [Webhooks](webhooks.md) |
| GET | `/api/webhooks/` | List configurations. | [Webhooks](webhooks.md) |
| GET | `/api/webhooks/{id}` | Read configuration and recent operational history. | [Webhooks](webhooks.md) |
| PUT | `/api/webhooks/{id}` | Replace configuration while preserving the saved secret. | [Webhooks](webhooks.md) |
| POST | `/api/webhooks/{id}/validate` | Validate the saved receiver with a signed HTTP request. | [Webhooks](webhooks.md) |
| DELETE | `/api/webhooks/{id}` | Delete the configuration at its current revision. | [Webhooks](webhooks.md) |

### Content operations

| Method | Endpoint | Purpose | Reference |
| --- | --- | --- | --- |
| POST | `/api/import/group/review` | Parse CSV and return valid recipients alongside rejected rows. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/import/group` | Parse CSV file parts and return recipients. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/import/email` | Extract subject, text, and HTML from an RFC 822 message. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/import/site` | Fetch and transform an HTTP(S) page into editable source. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/import/page-bundle` | Import a base64-encoded ZIP containing index.html and its local assets. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/util/preview` | Render email HTML with sample recipient values without adding an open tracker. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/pages/preview` | Prepare a landing-page preview with the managed collector in nontracking mode. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/pages/validate` | Return landing-page diagnostics without a prepared HTML document. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/learning-pages/preview` | Prepare static learning HTML and return diagnostics. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/learning-pages/validate` | Prepare static learning HTML and return diagnostics. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/util/test_connection` | Test SMTP connectivity and authentication without sending a message. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| POST | `/api/util/send_test_email` | Send a saved template and page to one explicit test recipient. | [Imports, previews, and test sends](imports-previews-and-test-sends.md) |
| GET | `/api/starter-templates/` | List the bundled catalog metadata. | [Starter templates](starter-templates.md) |
| GET | `/api/starter-templates/{catalog_id}` | Read a catalog entry and its ordinary resource-creation payload. | [Starter templates](starter-templates.md) |

### Campaigns

| Method | Endpoint | Purpose | Reference |
| --- | --- | --- | --- |
| POST | `/api/campaigns/prepare` | Resolve selections, deduplicate the audience, validate the destination, and render a sample without creating an execution. | [Campaigns](campaigns.md) |
| POST | `/api/campaigns/` | Create an execution using the reviewed resource selections. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/` | List complete execution records. Prefer overview or directory when full content and timelines are unnecessary. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/{id}` | Read one execution and its recorded audience and timeline. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/{id}/context` | Read operational context without full content or recipient history. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/overview` | Read a bounded page of execution contexts. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/directory` | List saved drafts and executions in one bounded directory. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/summary` | List execution metadata and progression counts. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/{id}/summary` | Read metadata and progression counts for one execution. | [Campaigns](campaigns.md) |
| GET | `/api/campaigns/{id}/draft` | Return editable duplication input without persisting a saved draft. | [Campaigns](campaigns.md) |
| POST | `/api/campaigns/test` | Send a prepared sample to one explicit recipient without creating an execution. | [Campaigns](campaigns.md) |
| POST | `/api/campaigns/{id}/complete` | Close the execution and stop pending delivery work. | [Campaigns](campaigns.md) |
| DELETE | `/api/campaigns/{id}` | Delete the execution and its retained campaign data. | [Campaigns](campaigns.md) |
| POST | `/api/campaign-drafts` | Persist an editable configuration. | [Campaign drafts](campaign-drafts.md) |
| GET | `/api/campaign-drafts` | List at most 50 saved drafts after the numeric cursor. | [Campaign drafts](campaign-drafts.md) |
| GET | `/api/campaign-drafts/{id}` | Read a saved configuration. | [Campaign drafts](campaign-drafts.md) |
| PUT | `/api/campaign-drafts/{id}` | Replace configuration and editing step at the supplied revision. | [Campaign drafts](campaign-drafts.md) |
| POST | `/api/campaign-drafts/prepare` | Review launch input for a draft without saving or launching it. | [Campaign drafts](campaign-drafts.md) |
| POST | `/api/campaign-drafts/{id}/launch` | Launch the unchanged saved configuration at its reviewed revision. | [Campaign drafts](campaign-drafts.md) |
| DELETE | `/api/campaign-drafts/{id}` | Remove an editable draft at the supplied query revision. | [Campaign drafts](campaign-drafts.md) |
| GET | `/api/campaigns/{id}/delivery` | Read the queue state, attempt summaries, and recent campaign activity. | [Delivery and history](delivery-and-history.md) |
| GET | `/api/campaigns/{id}/recipients/{recipient_id}/history` | Read one recipient’s attempt history and editable event records. | [Delivery and history](delivery-and-history.md) |
| POST | `/api/campaigns/{id}/delivery/{recipient_id}` | Apply an explicit recovery decision to the reviewed delivery generation. | [Delivery and history](delivery-and-history.md) |

### Results

| Method | Endpoint | Purpose | Reference |
| --- | --- | --- | --- |
| GET | `/api/campaigns/{id}/results` | Read filtered counts, recipient rows, events, and time buckets. | [Results and exports](results-and-exports.md) |
| GET | `/api/campaigns/{id}/export` | Download the filtered snapshot. | [Results and exports](results-and-exports.md) |
| POST | `/api/campaigns/{id}/recipients/{recipient_id}/events` | Add an operator observation to a recipient history. | [Event corrections](event-corrections.md) |
| GET | `/api/overview` | Read recent campaigns, inventory counts, and operational attention items. | [Overview](overview.md) |
| GET | `/api/overview/posture` | Read recipient engagement for an SMTP-acceptance cohort. | [Overview](overview.md) |

### Administration

| Method | Endpoint | Purpose | Reference |
| --- | --- | --- | --- |
| GET | `/api/settings` | Read company policy and its revision. | [Company settings](company-settings.md) |
| PUT | `/api/settings` | Replace company policy at the current revision. | [Company settings](company-settings.md) |
| POST | `/api/campaigns/complete-all/preview` | Obtain a confirmation token for closing the current active campaign set. | [Company settings](company-settings.md) |
| POST | `/api/campaigns/complete-all` | Close the campaign set identified by the preview token. | [Company settings](company-settings.md) |
| POST | `/api/users/` | Create an application account. | [Accounts and keys](accounts-and-keys.md) |
| GET | `/api/users/` | List accounts visible to an administrator. | [Accounts and keys](accounts-and-keys.md) |
| GET | `/api/users/{id}` | Read the specified account. | [Accounts and keys](accounts-and-keys.md) |
| PUT | `/api/users/{id}` | Replace account configuration while preserving its password. | [Accounts and keys](accounts-and-keys.md) |
| DELETE | `/api/users/{id}` | Delete another account. Self-deletion and removal of the last available administrator are rejected. | [Accounts and keys](accounts-and-keys.md) |
| POST | `/api/reset` | Rotate the authenticated account’s API key. | [Accounts and keys](accounts-and-keys.md) |
