---
title: Correct events
description: Add, edit, or remove recipient observations and understand the effects.
sidebar:
  order: 33
---

You can correct individual observations in ongoing or completed campaigns if your account has permission to modify campaigns.

Corrections change the records used by summaries, charts, and exports. Previous values and deleted events cannot be restored through the application.

## Open the event editor

1. Open the campaign's **Sending** tab.
2. Expand the recipient row to inspect their history.
3. Choose **Edit events**.
4. Find the event you want to correct, loading older history if necessary.

Delivery attempts appear alongside activity, but they have separate controls. Editing an interaction does not retry an email or change the campaign's lifecycle.

## Change an event

Choose **Edit**, update the occurrence time, event type, or available submission-presence indicators, and choose **Save changes**.

Time input uses your displayed zone. For a historical correction, enter the actual occurrence time rather than accepting the current-time default.

Submission indicators describe presence only: **Entered**, **No entry detected**, or **Not recorded**. Do not enter passwords, usernames, or other submitted values; the editor does not collect them.

Browser and device observations are read-only and remain attached to the original request.

## Add or remove an observation

Choose **Add event** to record an observation manually. Set its type and time, then save. The new event is labeled **Manually recorded**.

Choose **Delete** to remove one event, then read and confirm the deletion. Five click events become four when one is deleted; the recipient still counts once as having clicked. Deleting their last recorded click removes that direct observation, although a progression summary can still infer a clicked stage from a submission.

Manual changes do not send participant-event webhooks, resend mail, or reopen a completed campaign. Automatic recording can continue to add independent events while the campaign is active.

## Read origin and edited labels

**Origin** identifies how an event was first created. **Edited** means its values were subsequently changed.

A manually added event is not automatically marked edited. Editing it later preserves its manual origin and marks it edited.

## Recover a failed change

Validation or conflict errors keep your input. If someone else changed the event, preserve your intended changes, cancel, and reopen it to inspect the current version.

If the save outcome is unknown, keep the editor open and use **Retry**. Recreating an event from scratch could add a second observation.

After a correction, check the recipient history and the affected [results](campaign-analysis.md).
