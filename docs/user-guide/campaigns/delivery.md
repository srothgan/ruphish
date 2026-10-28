---
title: Understand delivery and recover failures
description: Distinguish waiting, failed, blocked, and uncertain SMTP outcomes.
sidebar:
  order: 5
---

Open the campaign's **Sending** view to inspect recipient delivery state and attempt history. **Email sent** means the configured mail server accepted the message; it does not confirm final delivery to a person's inbox. The donut counts recipients once by their current delivery state, not once per attempt. Expand a recipient row for the timeline and delivery-attempt details; **Edit events** opens the [correction dialog](../results/correcting-events.md).

Administrative history names the action taken: retry requested, resend requested despite uncertainty, or marked as sent manually. These actions are separate from engagement observations.

| State | Meaning and next action |
| --- | --- |
| Scheduled/queued | Waiting for its due time, worker capacity, or domain readiness |
| Preparing email / Sending email | An attempt is being composed or submitted |
| Email sent | The mail server accepted the email; inbox delivery is not confirmed |
| Retry scheduled | A retryable failure has a persisted next attempt time |
| Action required | Profile or policy needs attention; correct it before retrying |
| Sending failed | Permanent rejection or exhausted automatic retries; inspect the cause |
| Sending unconfirmed | Acceptance may have happened; inspect the receiving system before deciding |
| Cancelled | Work is no longer scheduled for submission |

## Automatic retries

Retryable failures allow eight retries after the initial attempt. Delays grow exponentially from two minutes with bounded positive jitter. Permanent rejection is not automatically retried. Profile connection/TLS/authentication problems can also trigger cooldown behavior.

Expired domain evidence pauses new work without consuming SMTP retries. Correct the [domain verification problem](../domains.md), then inspect whether delivery resumes.

## Unknown outcomes need a decision

A connection can fail after the receiving SMTP server has accepted a message, or persistence can fail after submission. An unknown outcome is not proof of failure. Ruphish does not automatically resend it.

Inspect the attempt's message identifier, protocol stage, SMTP status if present, and the receiving service's evidence. Choose **Mark as sent** only when the mail-server records confirm acceptance. Explicitly choosing to resend an unknown attempt accepts the possibility of duplicate mail. Failed/blocked deliveries have a separate retry action.

Actions check the current attempt generation. If the row changed, refresh before deciding again; do not replay a stale recovery action blindly.

## Restart and completion

Abandoned preparation can be reclaimed after its lease expires. Interrupted submission can become uncertain instead. Completion prevents further ordinary queued work, but cannot recall an external submission already in progress. A late confirmed acceptance may still settle that attempt.

For installation-wide outages, see [operational troubleshooting](../../administration/troubleshooting.md). For automation, see [campaign delivery actions](../../api/campaigns.md#delivery-and-uncertain-outcomes).

Delivery errors show a stable application code together with a translated explanation and next action. Include the code when diagnosing a problem. An SMTP response number in the attempt details is separate from that application code.
