---
title: Process reported emails
description: Configure an IMAP mailbox and understand matching, polling, and acknowledgement behavior.
sidebar:
  order: 11
---

Reporting a suspicious message is a useful outcome. Ruphish can monitor a dedicated IMAP folder and correlate forwarded campaign messages with their original recipient.

## Configure and validate

If reporting mailboxes are disabled in company settings, an administrator must enable the feature first. Open **Resources → Reporting mailboxes**. Enter a name, host, port, service username/password, folder, and polling interval. Implicit TLS is the default; disabling it requires STARTTLS, not plaintext authentication. Standard username/password IMAP is supported; OAuth onboarding is not provided.

Save, then choose **Validate connection**. Saving alone does not test the credentials. Validation checks TLS, authentication, and folder access without importing reports. Enable polling and use **Check mailbox now** to queue a check; an accepted check is not a completed poll.

Certificate and hostname validation remain enabled. An installation operator can supply trusted roots for a private mail service. Do not solve a trust failure by requesting an insecure certificate bypass.

## Send a report

The first successful poll establishes a checkpoint and watches new arrivals; existing mail is left untouched. Forward a campaign message inline or as a `message/rfc822` attachment after that checkpoint.

A report must contain one distinct recipient-ID/origin pair from a campaign owned by the mailbox's account. Repeated references to the same pair are allowed. Multiple different campaign links make correlation ambiguous. Completed campaigns can still receive mailbox reports.

An optional From-domain restriction is only a filter; it does not authenticate the reporter.

## Polling and recovery

Intervals start at 30 seconds. Failed polls back off up to one hour. The detail view shows the active check, last result, next check, and recent history.

Reports are committed before the message is marked seen. If acknowledgement fails, restart/retry does not create another report event for the same processed UID. Unmatched messages remain in the mailbox without blocking later arrivals. Bodies over 2 MiB and messages exceeding parsing/correlation budgets are skipped or rejected with a diagnostic.

If the server changes UIDVALIDITY, processing stops. **Resume with new arrivals** establishes a new checkpoint and leaves existing messages untouched; campaign history remains.

Optional deletion uses UID-scoped removal only when the server supports UIDPLUS; it never issues mailbox-wide expunge. Raw message bodies, subjects, and reporter addresses are not stored in reporting history. Check the [API reference](../api/reporting-mailboxes.md) for automation and legacy IMAP settings behavior.

Installation administrators can disable this feature in [company settings](../administration/settings.md). Disabled features disappear from navigation, while direct page visits explain the disabled state. Configurations and history are retained; network actions return `409` with `FEATURE_DISABLED`.
