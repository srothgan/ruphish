---
title: Company settings
description: Control domain verification, reporting mailboxes, webhooks, and emergency campaign closure.
sidebar:
  order: 4
---

Administrators with `modify_system` permission manage installation-wide policy at **Settings** (`/settings`). Changes apply to every account and survive restart. The page also retains the link to the [starter-template catalog](../user-guide/starter-templates.md).

## Choose enabled features

Domain verification always starts off when settings are introduced. Reporting mailboxes start on only if mailbox configurations already exist; webhooks start on only if webhook configurations already exist. With no existing configurations, both start off. Presence determines these initial global switches, including when the existing configurations are individually disabled; each configuration retains its own enabled state. Later administrator changes to the global switches are saved and survive restart. Enabling reporting or webhooks lets regular users create and configure integrations without individual administrator approval. Mailbox configuration remains creator-scoped; webhook configurations also belong to their creator, and their deliveries include events from all campaign creators.

When verification is required, only DNS hostnames over HTTPS are eligible. HTTP and IP origins are available when verification is off.

With verification off, domains remain required, owned resources with individual enablement and HTTP/HTTPS origin validation. Automatic and manual verification checks stop. The UI hides verification controls and shows verification as not required. Campaign selection and delivery no longer require verification evidence; origin matching and ownership checks still apply.

When verification is enabled, successful evidence normally expires after 24 hours and rechecks run approximately every six hours. **Expire verification** is a saved option that has no effect while verification is off. Turning expiry off permits successful evidence to remain valid until a failed check or domain disablement revokes it; rechecks continue. Shortening validity can immediately make domains ineligible for new delivery. Expiry alone preserves existing participant links. See [domains](../user-guide/domains.md).

Disabling reporting mailboxes or webhooks hides the navigation entry and shows an explanatory message when its URL is visited directly. Saved configurations, service credentials, checkpoints, and history remain available. Creation, configuration saves, and network validation/check/resume actions are rejected while disabled. Authorized API reads and deletion remain available. Mailbox polling stops. Disabling webhooks cancels pending deliveries; events occurring while disabled do not create a backlog to replay when re-enabled. An external operation already authorized before the change may finish.

Save the policy and confirm the warning. Concurrent changes cause a revision conflict. Refresh loads the current policy into a form with no edits. If you have unsaved edits, refresh keeps them and asks you to discard them before using the newer policy. Choose **Discard edits**, review the current values, and reapply any intended changes before saving again. A failed refresh keeps your draft available.

## Close all active campaigns

The danger section reviews every active launched campaign, including scheduled campaigns across all accounts. Drafts are excluded. Check the displayed campaign and unsent-delivery counts, type `CLOSE ALL`, and confirm within five minutes. If the active campaign set changes before confirmation, obtain a new review.

Closure uses the existing **Completed** state. It cancels unsent work, disables participant links, and preserves campaign history. It cannot recall submitted mail or guarantee cancellation of an SMTP operation already underway. This is an immediate action, not a persistent pause: future campaigns can still launch. Retrying the same confirmed operation returns its saved result, including after restart, without closing campaigns created later.

For automation, see the [settings API](../api/settings.md).
