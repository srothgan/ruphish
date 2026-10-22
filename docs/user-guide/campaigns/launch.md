---
title: Review and launch a campaign
description: Check selections, recipients, tracking policy, and delivery readiness before launch.
sidebar:
  order: 3
---

Before launching, save your group, template, landing page, sending profile, and domain. Use a [test message](../sending-profiles.md#test-connection-versus-send-test) to inspect the output in a controlled inbox.

1. Create or resume a draft and select the resources.
2. Choose the verified public origin and any path beneath it.
3. Set an immediate or future schedule.
4. Review the resolved audience, personalized preview, and effective tracking/redirect settings.
5. Choose **Launch campaign** or **Schedule campaign**.

## Check the audience

The launch audience is resolved from the selected groups. Review shows the number of unique recipients, duplicate entries, and conflicts where the same address carries different attributes. First-occurrence precedence makes group order meaningful; inspect the retained details rather than assuming every group row becomes a separate message.

Resource revisions are checked at launch. If a selected resource changed after review, refresh the review so you approve the current content. Missing resources or an ineligible domain must be corrected first.

## Check the policy

Landing-page tracking settings supply defaults, but review shows the campaign's effective policy. Submission tracking records events; optional password tracking records presence only. A launch does not enable entered-value capture. See [participant data](../participant-data.md).

## If the response is lost

Launching a saved draft creates the campaign transactionally. Retrying that same draft launch resolves to its existing campaign rather than creating another one. Refresh the saved draft to find the linked execution.

This launch identity does not promise exactly-once email delivery. Read [delivery recovery](delivery.md) before acting on an uncertain SMTP outcome. The legacy API has its own [idempotency mechanism](../../api/campaigns.md).
