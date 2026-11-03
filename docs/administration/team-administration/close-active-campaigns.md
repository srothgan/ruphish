---
title: Close active campaigns
description: End active and scheduled campaigns across the shared workspace while preserving their history.
sidebar:
  order: 14
---

Use **Close all active campaigns** when campaigns across the installation must end, for example when an exercise is cancelled. You need an Administrator account. To end just one campaign, use [Complete a campaign](../../user-guide/campaigns/complete-or-delete-a-campaign.md).

Closing campaigns marks them **Completed**, cancels unsent delivery work, and stops their tracking and landing-page links from accepting further interactions. Results and campaign history remain available. Existing learning-page links can still open while their domain remains eligible.

This action cannot recall email already sent or reliably stop an SMTP submission already in progress. It is not a pause: completed campaigns cannot resume sending, and users can still launch new campaigns afterward.

## Review and close

1. Open **Administration → Company settings**.
2. Find **Close all active campaigns** and request the review.
3. Check the campaign and pending-delivery counts. The review includes active and scheduled launched campaigns from every account. It excludes drafts.
4. Type `CLOSE ALL` and confirm within five minutes.
5. Check the result and the campaign list. The affected campaigns should show **Completed**.

If the active campaign set changes or the review expires, request a new review before confirming. If the response is interrupted, retry the same reviewed action; Ruphish keeps its result so that the retry does not close campaigns launched afterward.

For a planned server outage, [stop the service](../hosting-and-maintenance/index.md) instead. Completing campaigns changes their lifecycle permanently; stopping the service allows its unfinished work to resume when it starts again.
