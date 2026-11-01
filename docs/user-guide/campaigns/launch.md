---
title: Review and launch a campaign
description: Check selections, recipients, tracking policy, and delivery readiness before launch.
sidebar:
  order: 3
---

Before launching, save your group, template, landing page, sending profile, and domain. Use a [test message](../sending-profiles.md#test-connection-versus-send-test) to inspect the output in a controlled inbox.

1. Create or resume a draft and select the resources.
2. Choose an enabled shared public origin and any path beneath it. HTTP and HTTPS support fixed DNS names and IP addresses. When verification is required in [company settings](../../administration/settings.md), only verified DNS hostnames over HTTPS are eligible.
3. Set an immediate or future schedule.
4. Review the resolved audience, personalized preview, and effective tracking/redirect settings.
5. Choose **Launch campaign** or **Schedule campaign**.

## Check the audience

The launch audience is resolved from the selected groups. Review shows the number of unique recipients, duplicate entries, and conflicts where the same address carries different attributes. First-occurrence precedence makes group order meaningful; inspect the retained details rather than assuming every group row becomes a separate message.

Revisions of the selected groups, email template, landing page, learning page and sending profile are checked at launch. If one changed after review, refresh the review so you approve the current content. Missing resources or an ineligible domain must be corrected first.

Choose **Custom redirect** or a reusable **Learning page** as the post-submission destination. The destination remains fixed after launch. See [learning pages](../learning-pages.md) for authoring, public availability and content-edit behavior.

## Check the policy

Submission and password-presence tracking are always enabled. Review shows the campaign's redirect and advisory page warnings. A launch does not enable entered-value capture. See [participant data](../participant-data.md).

## If the response is lost

Launching a saved draft creates the campaign transactionally. Retrying that same draft launch resolves to its existing campaign rather than creating another one. Refresh the saved draft to find the linked execution.

This launch identity does not promise exactly-once email delivery. Read [delivery recovery](delivery.md) before acting on an uncertain SMTP outcome. The legacy API has its own [idempotency mechanism](../../api/campaigns.md).

## Choose email-open tracking

**Track email opens** defaults to enabled. Turn it off before launch to omit Ruphish tracking images and ignore requests to the campaign tracking endpoint. Clicks, submissions, and reports remain independent. The choice is saved with the draft, shown in review, used by campaign test sends, and fixed when launched. Disabled open metrics display **Not tracked** or **-** rather than 0%.

## What stays fixed after launch

Launching or scheduling a campaign saves its recipients, email subject/body and attachments, sender identity and custom headers, landing-page content, selected learning-page content, URL, schedule, and tracking/redirect policy. Editing a source resource affects later campaigns, not this campaign's queued messages, retries, participant pages, or historical content. Copies select current source resources and require a fresh review.

SMTP host, authentication and TLS settings remain live so connection problems can be repaired and credentials rotated. A profile edit during preparation cancels that prepared attempt and safely prepares it again with the current transport settings and the campaign's frozen content. Domain enablement and verification, account authorization, and campaign closure remain live controls. Standalone learning-page links continue to show the current resource.

Gophish imports freeze the resource content available at conversion. Those copies cannot establish what an older campaign originally sent if its resources had already changed or been deleted.
