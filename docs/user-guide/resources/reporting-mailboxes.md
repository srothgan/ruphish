---
title: Reporting mailboxes
description: Connect an inbox to record campaign messages that recipients report as phishing.
sidebar:
  order: 17
---

A reporting mailbox lets Ruphish record when a recipient reports a campaign email. Ruphish checks a dedicated IMAP folder, matches forwarded messages to campaign recipients, and adds report events to their results.

This mailbox receives reports. Your [sending profile](sending-profiles.md) is the separate connection used to send campaign emails.

## Before you begin

An administrator must enable reporting mailboxes in company settings. After that, users with the relevant resource permissions can configure them without individual administrator approval.

Prepare a dedicated account or folder and obtain its IMAP hostname, port, username, and password. Ruphish supports password-based IMAP connections; it does not provide OAuth onboarding.

## Connect the mailbox

1. Open **Resources → Reporting mailboxes** and choose **Create**.
2. Enter a name, the **IMAP hostname**, and the port supplied by your provider.
3. Use the provider's TLS settings. Keep certificate verification enabled.
4. Enter the service account username and password.
5. Enter the folder to monitor, such as `INBOX`, and a check interval of at least 30 seconds.
6. Clear **Enabled** while you check the setup, save the configuration, and choose **Test connection**.
7. After the connection test succeeds, edit the mailbox, select **Enabled**, save, and choose **Check mailbox now**.

The connection test checks connectivity, authentication, and folder access without importing reports. **Check mailbox now** queues report processing; inspect the last outcome, last successful check, and history after it finishes.

If a private mail server uses a private certificate authority, ask the installation operator to configure trust. **Skip certificate verification** keeps encryption but stops checking the server's identity.

## Tell recipients how to report

Have recipients forward the original campaign message to the monitored mailbox, either inline or as an attached email. Forward one original campaign message at a time.

The forwarded content must retain a campaign link that identifies its original recipient and campaign address. A thread containing several recipients' campaign messages is ambiguous; send separate new forwards instead.

Test the process with a campaign sent to your own inbox. Forward that message, run a mailbox check, and inspect the recipient's **Reported** activity.

A report is attributed to the original campaign recipient. Forwarding is not proof of the reporter's identity. The optional sender-domain filter narrows accepted messages but does not authenticate the sender.

Reports for completed campaigns do not add new report events.

## Understand which messages are checked

When you create a mailbox, Ruphish uses the earliest recorded send among unfinished campaigns as the starting point for its initial scan. If no such send exists, it starts with new arrivals. Initial date-based discovery can also inspect messages from the preceding day.

After the initial scan, Ruphish remembers its progress. Starting another campaign does not make it rescan all old messages. An unmatched message does not prevent later messages from being processed.

Matched reports are recorded before their messages are marked as read. A failed acknowledgement can be retried without recording the same processed message twice.

If you enable **Delete successfully matched messages after processing**, matched messages can be removed when the server supports safe per-message deletion. Leave this off when you want to retain the original reports in the inbox.

## Resolve processing problems

| Problem | Next step |
| --- | --- |
| Connection test fails | Check IMAP access, credentials, folder name, port, and certificate settings. |
| A report is ambiguous | Forward each original message separately, without quoted campaign messages for other recipients. |
| No match is found | Check that the original campaign link survived forwarding and belongs to this installation. |
| The campaign is completed | Its results are closed to new mailbox reports. |
| Mailbox identifiers changed | Use **Resume with new arrivals** after reviewing the message. Existing messages will not be reprocessed. |
| A check failed temporarily | Inspect the history and next check time. Failed checks retry with increasing delays. |

## Edit, disable, or delete a mailbox

Leave the password blank during an edit to retain it. Enter a new password to replace it.

Disabling the mailbox pauses scheduled checks and preserves its progress. Ordinary settings changes preserve progress too. Changing the host, port, username, or folder identifies a different mailbox and starts monitoring new arrivals.

Deleting the configuration stops monitoring that mailbox. Reports already recorded remain part of campaign history. Use the [reporting mailbox API](../../api/reporting-mailboxes.md) for integration details.
