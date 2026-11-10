---
title: Monitor sending
description: Understand delivery states and recover failed or unconfirmed sends.
sidebar:
  order: 23
---

Open a launched campaign's **Sending** tab to see delivery progress. Each recipient has a current delivery state; expanding the row shows their interaction history and delivery attempts.

**Email sent** means the mail server accepted the message, or an operator confirmed acceptance using mail-server records. It does not confirm arrival in the recipient's inbox.

## Understand the states

| State | Meaning and next step |
| --- | --- |
| Scheduled or queued | Waiting for the due time, capacity, domain readiness, or sending-profile recovery. Check the schedule and any displayed reason. |
| Preparing email | Ruphish is preparing an attempt. |
| Sending email | Submission is in progress; acceptance is not yet confirmed. |
| Email sent | The mail server accepted the email. Investigate with the provider if it is missing from the inbox. |
| Retry scheduled | A temporary failure occurred and another attempt is scheduled. |
| Action required | A configuration or policy problem needs correction before sending can continue. |
| Sending failed | Sending was rejected, content preparation failed, or automatic retries were exhausted. Inspect the cause. |
| Sending unconfirmed | The message may have been accepted. Check the provider before choosing a recovery action. |
| Cancelled | Pending work was cancelled. Previously submitted messages cannot be recalled. |

The default **Needs attention first** order brings problems to the top. The progress chart counts each recipient once by their current state; repeated attempts do not add recipients.

## Investigate a problem

1. Expand the affected recipient's row.
2. Read the latest attempt's explanation and application error code.
3. Check the SMTP response and message identifier when available.
4. Correct the indicated connection, account, domain, or content problem.
5. Use the recovery action appropriate to the state.

An application error code and an SMTP response number describe different things. Keep both when asking your mail or Ruphish operator to investigate.

Attempt details distinguish `Configuration`, `Dns`, `DestinationPolicy`, `Connect`, `Tls`, and `Authentication`. For example, `smtp_destination_denied` means the server's outbound policy rejected the resolved address; it does not mean that the provider rejected your password. See [sending-profile troubleshooting](../resources/sending-profiles.md#resolve-a-failed-test).

Connection, credential, and certificate changes in the [sending profile](../resources/sending-profiles.md) can help existing campaigns. Editing the source email or landing page does not replace a launched campaign's saved content.

## Let temporary failures retry

Retryable failures allow up to eight retries after the initial attempt, with increasing delays. Permanent rejection is not retried automatically.

A blocked sending profile holds other queued recipients using that profile, including recipients in other campaigns. Other profiles can continue. Saving corrected profile settings allows ordinary queued work to resume automatically; the recipient already marked **Action required** still needs **Retry**. Restarting the server does not clear the block.

Expired domain verification can pause sending without using SMTP retries. Correct the [domain problem](../resources/domains.md), then check whether work resumes.

For failed or blocked deliveries, fix the cause before choosing the available retry action. Repeated retries cannot correct invalid account settings or a provider's rejection policy.

## Resolve an unconfirmed send

An interrupted connection can leave Ruphish unsure whether the mail server accepted a message. Ruphish does not automatically resend it.

1. Check the provider's logs using the message identifier and attempt time.
2. If acceptance is confirmed, choose **Mark as sent**.
3. If you decide to send again, choose **Resend despite uncertainty** and review the confirmation.

Resending can deliver a duplicate. Marking as sent records your decision; it does not submit another message.

If the recipient's state changed while you were investigating, refresh it before acting on the current attempt.

## After a restart or completion

Ruphish retains queued work and attempt history. An interrupted preparation can be retried; an interrupted submission may need the unconfirmed-send workflow.

Completing a campaign stops future ordinary sending, but an attempt already in progress may still finish. Inspect its final outcome rather than assuming completion recalled the message.

For broader failures, see [Troubleshooting](../troubleshooting.md).
