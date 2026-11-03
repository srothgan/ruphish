---
title: Resources
description: Prepare the reusable content, audiences, and connections your campaigns need.
sidebar:
  order: 10
---

Resources are the building blocks you select when creating a campaign. Prepare them once, then reuse them in later campaigns.

| Resource | Its role |
| --- | --- |
| [Recipient groups](recipient-groups.md) | The people you want to include. |
| [Sending profiles](sending-profiles.md) | The mail account Ruphish uses to send messages. |
| [Email templates](email-templates.md) | The message and any attachments recipients receive. |
| [Landing pages](landing-pages.md) | The page recipients open from the message. |
| [Learning pages](learning-pages.md) | An optional explanation shown after a submission. |
| [Domains](domains.md) | The web addresses that serve your campaign pages. |
| [Reporting mailboxes](reporting-mailboxes.md) | Optional inboxes that receive reported phishing messages. |
| [Webhooks](webhooks.md) | Optional connections that send events to other systems. |

A campaign needs recipient groups, an email template, a landing page, a sending profile, and an eligible domain. Learning pages, reporting mailboxes, and webhooks support additional workflows.

For ready-made content, see [Add starter templates](../add-starter-templates.md). For personalized greetings and links, see [Personalize content](../personalize-content.md).

## Create and reuse resources

Open **Resources** in the sidebar and choose the resource type. Create an item, check it, and save it before selecting it in a campaign.

Recipient groups, email templates, landing pages, learning pages, and sending profiles support duplication and JSON export/import. Duplication creates a separate editable copy. An import opens a draft for review; it becomes a saved resource only after you choose **Create**. Exported sending profiles do not contain their saved passwords.

Resources are shared with colleagues who have access. Groups, email templates, landing pages, learning pages, and sending profiles require unique names within their type across the installation. Use **Duplicate** when you need a separate variation rather than changing a resource that others use.

## Edit shared work

Open a resource and choose **Edit**. Save your changes before leaving. If someone else saved a newer version, Ruphish rejects your stale save instead of overwriting their changes. Keep your unsaved text, open the current resource in another tab, and reconcile the differences before trying again.

Draft campaigns use the current resources when reviewed. Launched campaigns retain their selected recipients and authored content, including campaigns scheduled for later. Sending connection settings and domain availability can still affect ongoing delivery; see their individual guides.

## Remove a resource

Open the resource, choose **Delete**, and review the confirmation. Ruphish prevents deletion of groups, templates, pages, and sending profiles still referenced by unfinished campaigns. Domains have a stricter rule: they cannot be deleted while any retained campaign references them, including completed campaigns.

Drafts do not reserve resources indefinitely. If a selected resource is deleted, replace that selection before launching the draft.
