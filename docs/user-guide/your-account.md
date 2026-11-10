---
title: Your account
description: Manage your password, display preferences, and API access.
sidebar:
  order: 50
---

Open your username at the bottom of the sidebar to find **My account**, **Settings**, and **Sign out**.

Your account's role and permissions control what you can do. Resources and campaigns are shared according to those permissions.

## Opening Ruphish

Ruphish shows a loading card while checking your session or moving between your workspace and sign-in. If this takes longer than usual, the card updates its message.

If the session check fails, choose **Try again** to repeat the check. If a page transition stalls, choose **Continue** when it appears. These actions do not resubmit your password or repeat sign-out. An expired session returns you to sign-in with your destination preserved.

## Change your password

1. Open **My account** and choose **Change password**.
2. Enter your current password.
3. Enter and confirm the new password.
4. Save the change.

The new password must differ from the current one. Ruphish validates its length and confirmation and displays any problem beside the form.

A successful password change revokes your other sessions. Initial account setup uses a separate required password-change screen.

If you cannot sign in, ask an authorized administrator to reset your account. See [account administration](../administration/team-administration/shared-access-and-permissions.md).

## Choose language and time settings

Open **Settings** from your account menu. Choose the interface language, date/time formatting, and time zone independently.

For example, you can use English interface text with German date formatting and a Berlin time zone. Use the preview to check the result and save. Reset an override to follow the browser again.

These preferences follow your account across devices. Changing the display zone does not reschedule existing campaigns, but it affects how you enter and read local times. See [Scheduling](campaigns/schedule-a-campaign.md).

## Use or replace your API key

**My account** lets you reveal or copy your API key. It acts with your account's permissions.

Replace the key when it should no longer be used. The old key stops working immediately, so update any integrations that depend on it.

Keep the key out of shared screenshots and messages. For request examples, see [API authentication](../api/authentication.md).

## Sign out

Choose **Sign out** from the account menu. If you have unsaved work, review the discard prompt before leaving.

Signing out ends that login. Other independently authenticated browsers can remain signed in unless an account action revokes their sessions.
