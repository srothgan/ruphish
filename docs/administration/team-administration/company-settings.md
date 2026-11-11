---
title: Company settings
description: Set domain-verification policy and enable reporting mailboxes and webhooks for your team.
sidebar:
  order: 13
---

Open **Administration → Company settings** with an Administrator account. These settings apply to everyone using this installation and stay in effect after a restart.

Change the settings you need, choose **Save changes**, and review the confirmation before applying them. If another administrator has saved changes first, refresh and review the current settings. Use **Discard edits** when you want to replace your unsaved changes with the saved version.

## Require a successful sending-profile test

**Require successful sending-profile test** defaults to off. Enable it to require an accepted fixed-content email test using a sending profile's current configuration before a new campaign can launch, including a campaign scheduled for future delivery. Drafts can still be saved before the profile qualifies. Use the profile view's **Send test email** action to obtain qualifying evidence; a connection-only check or a template/campaign test does not qualify.

Changing SMTP settings, credentials, sender address, certificate policy, or additional headers requires a new successful test. Renaming a profile does not. A later failed, interrupted, or unknown operational test revokes readiness until a new profile email test succeeds. An active test also holds new launches until its result is known. Tests have no automatic expiry or scheduled rechecks.

This policy governs new launches. Changing the policy or later test results does not pause campaigns already launched. SMTP acceptance does not verify sender ownership or inbox delivery; inspect the test recipient's inbox separately. See [sending profiles](../../user-guide/resources/sending-profiles.md#check-the-connection-and-send-a-test) for the actions, result badges, and history.

## Require domain verification

Domain verification checks that a registered campaign hostname is under your control and reaches this Ruphish installation over HTTPS. Enable it when your team uses hosted campaign domains. Leave it off for the local getting-started setup, which uses an HTTP address on your own computer.

| Setting | Effect |
| --- | --- |
| **Require domain verification** | Campaign domains must use a DNS hostname over HTTPS and pass verification before they can be used for new delivery. HTTP and IP-address origins cannot meet this requirement. |
| **Successful verification expires** | Successful checks remain valid for the configured period. The default is 24 hours. |
| **Recheck interval (seconds)** | Ruphish checks domains again periodically. The default is six hours. Rechecks continue even when expiry is disabled. |

The form expresses validity and recheck intervals in seconds: 24 hours is `86400`, and six hours is `21600`. Validity can range from one hour to 365 days; rechecks can range from five minutes to seven days. When expiry is on, leave enough time for another check before expiry. The form rejects intervals that are too close together.

Before enabling verification or shortening its validity period, review the team's domains. A domain that becomes ineligible can block new campaign delivery. Enabling verification can also stop existing links for domains that have never passed verification or whose latest check failed. Expired evidence alone preserves existing campaign links; a failed verification check or disabling the domain can also stop those links.

When verification is off, registered domains are still required and must be enabled. Ruphish stops verification checks and allows HTTP or HTTPS origins, including IP addresses. Follow [Domain setup](../../user-guide/resources/domains.md) for registration and verification, or ask the host to configure [network access and HTTPS](../hosting-and-maintenance/network-access-and-https.md).

## Enable reporting mailboxes

Enable **Reporting mailboxes** and save to let your team connect mailboxes that receive reports of suspicious email. Users can then add and manage shared mailboxes under **Resources → Reporting mailboxes**. Each mailbox has its own connection settings and enabled state.

Follow [Reporting mailboxes](../../user-guide/resources/reporting-mailboxes.md) to connect and test an account.

Turning the feature off stops polling and hides it from navigation. Saved accounts, credentials, and processing progress remain. Turn it back on to resume using those configurations; an individually disabled or paused mailbox still needs attention in its own settings.

## Enable webhooks

Enable **Webhooks** and save to let Ruphish send campaign events to an external service. Users with resource-editing permission can then configure shared endpoints under **Resources → Webhooks**, without separate approval for each endpoint.

An enabled endpoint receives events from campaigns across the whole installation. Choose a receiving service that is suitable for that shared scope. Follow [Webhooks](../../user-guide/resources/webhooks.md) for endpoint setup and testing, or the [webhook API contract](../../api/webhooks.md) when building a receiver.

Turning the feature off cancels pending webhook deliveries and prevents new ones. Saved configurations and history remain, but re-enabling the feature does not replay cancelled deliveries or events that occurred while it was off. A request already authorized or in flight may still finish.

## Feature defaults and disabled features

Fresh installations start with domain verification, reporting mailboxes, and webhooks off. When settings are introduced to an existing database, an existing mailbox or webhook configuration enables its corresponding global feature initially. Later administrator choices are preserved.

While an integration feature is off, users cannot create or save its configurations or run connection checks. Authorized API clients can still read or delete retained configurations. Disable an individual mailbox or webhook instead when you want to stop only that integration.

The settings page also links to the [starter-template catalog](../../user-guide/add-starter-templates.md) and provides the separate [Close active campaigns](close-active-campaigns.md) action. For automation, see the [settings API](../../api/company-settings.md).
