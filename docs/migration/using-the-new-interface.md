---
title: Using the new interface
description: Find existing Gophish tasks in Ruphish and review the changed campaign workflow.
sidebar:
  order: 50
prev: {"label": "Database migration", "link": "/docs/migration/database/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

After the host operator completes migration, open the Ruphish address and sign in with your existing account. Your permissions determine what you can view and change; ordinary campaign work does not require an administrator account.

## Find your tasks

| Gophish task | Ruphish location | What changes |
| --- | --- | --- |
| Manage users/groups of recipients | **Resources → Recipient groups** | Recipient groups are distinct from application accounts and are shared with the installation. |
| Manage email templates, landing pages, or sending profiles | **Resources** | Each resource shows its current configuration and attribution. Campaign launch snapshots recipients and authored content. |
| Configure campaign URLs | **Resources → Domains** | Register the origin and meet the installation's verification policy. |
| Configure reporting email or webhooks | **Resources → Reporting mailboxes / Webhooks** | Multiple shared configurations replace per-user mailbox settings; the administrator controls feature availability. |
| Create a campaign | **Campaigns** | Select resources, prepare/review the audience and content, and launch or save a draft. Overlapping recipient groups are deduplicated. |
| Investigate a campaign | Open the campaign's results and delivery views | Separate observations from inferred progression and SMTP delivery state. Unknown delivery outcomes need review before resending. |
| Change personal settings | Your account settings | Set your password, interface language, date/time format, and timezone. Presentation preferences do not move a campaign schedule. |
| Manage application accounts or company policy | Administration, when permitted | These are separate from recipient groups and shared campaign resources. |

The [user guide](../user-guide/index.md) follows the working sequence **Resources → Campaigns → Results**. Use it for detailed instructions; migration does not require repeating the first-campaign tutorial.

## Review your migrated work

1. Check the shared resource list and your existing campaign history. Confirm you are in the expected installation.
2. Preview the templates and landing pages you intend to reuse. Imported content now runs under Ruphish's rendering and submission rules.
3. Review campaign domains, sending profiles, reporting mailboxes, and webhooks before enabling external processing.
4. Inspect pending campaign deliveries. An imported `unknown` outcome is a request for evidence and a decision, not a failed-send instruction to retry immediately.
5. Check result definitions before comparing new reports with Gophish totals. An observed open, inferred open, and accepted SMTP delivery are different measurements.

See [What happens to your data](what-happens-to-your-data.md) for conversion details, [Campaigns](../user-guide/campaigns/index.md) for the workflow, and [Results](../user-guide/results/index.md) for reporting definitions.
