---
title: Process reported emails
description: Configure an IMAP mailbox and understand matching, polling, and acknowledgement behavior.
sidebar:
  order: 11
---

Reporting a suspicious message is a useful outcome. Ruphish can monitor a dedicated IMAP folder and correlate forwarded campaign messages with their original recipient.

Administrators enable reporting mailboxes in **Settings**. Once enabled, regular users can create and manage their own mailbox configurations without individual approval. Disabling the feature blocks creation, configuration saves, and network actions while retaining credentials and progress; authorized API reads and deletion remain available.

## Configure and validate

If reporting mailboxes are disabled in company settings, an administrator must enable the feature first. Open **Resources → Reporting mailboxes**. Enter a name, host, port, service username/password, folder, and polling interval. Implicit TLS is the default; disabling it requires STARTTLS, not plaintext authentication. Standard username/password IMAP is supported; OAuth onboarding is not provided.

Save, then choose **Validate connection**. Saving alone does not test the credentials. Validation checks TLS, authentication, and folder access without importing reports. Enable polling and use **Check mailbox now** to queue a check; an accepted check is not a completed poll.

Certificate and hostname validation are enabled by default. An installation operator can supply trusted roots for a private mail service. For a controlled test environment, enable **Skip certificate verification** in the mailbox connection settings. The connection remains encrypted, but the mail server's identity is not verified. The saved choice appears in mailbox details and applies to validation and polling. Turn it off to restore verification; changing it preserves the mailbox checkpoint and pending acknowledgements.

## Send a report

Forward a campaign message inline or as a `message/rfc822` attachment. When a mailbox is created, Ruphish selects the earliest actual Ruphish email-send observation among all non-completed campaigns in the installation as its initial scan boundary. The first successful poll uses that boundary to include existing reports. If no such send exists, it starts with new arrivals instead; there is no initial-scan setting.

IMAP searches use calendar dates, so initial discovery includes the preceding UTC date to account for mailbox time zones. It may inspect older messages in that window. Arrivals during discovery remain eligible for subsequent polling. After initialization, a durable UID checkpoint controls progress; campaign starts and completions do not recalculate it.

A report must contain one distinct recipient-ID/origin pair from a campaign in this installation, regardless of who created the campaign or mailbox. Repeated references to the same pair are allowed. Different pairs make correlation ambiguous, including when a forwarded thread quotes another recipient's campaign message or one email attaches several campaign messages. Such a message is rejected with `mailbox_ambiguous`; no report is attributed from it. Forward each original campaign message separately, without quoted messages or attachments for other recipients. Correcting the old message does not rewind the checkpoint; send separate new forwards. Batch attribution is deferred. Mailbox configuration remains creator-managed; matching another user's campaign grants no campaign editing permission.

Reports for completed campaigns are ignored with the operational reason `mailbox_campaign_closed`. They do not add events, change recipient results, or enqueue webhooks. The scanner advances its checkpoint and continues processing later messages.

An optional From-domain restriction is only a filter; it does not authenticate the reporter.

## Polling and recovery

Polling continues even when no campaign is active. Ordinary configuration edits and enable/disable changes preserve progress. Changing the host, port, username, or folder explicitly resets the mailbox identity and starts with new arrivals.

Intervals start at 30 seconds. Failed polls back off up to one hour. The detail view shows the active check, last result, next check, and recent history.

Reports are committed before the message is marked seen. If acknowledgement fails, restart/retry does not create another report event for the same processed UID. A report committed before campaign completion keeps its pending acknowledgement afterward. Unmatched messages remain in the mailbox without blocking later arrivals. Bodies over 2 MiB and messages exceeding parsing/correlation budgets are skipped or rejected with a diagnostic.

If the server changes UIDVALIDITY, processing stops. **Resume with new arrivals** establishes a new checkpoint and leaves existing messages untouched; campaign history remains.

Optional deletion uses UID-scoped removal only when the server supports UIDPLUS; it never issues mailbox-wide expunge. Raw message bodies, subjects, and reporter addresses are not stored in reporting history. Check the [API reference](../api/reporting-mailboxes.md) for automation and legacy IMAP settings behavior.

Installation administrators can disable this feature in [company settings](../administration/settings.md). Disabled features disappear from navigation, while direct page visits explain the disabled state. Configurations and history are retained; network actions return `409` with `FEATURE_DISABLED`.
