---
title: Manage your account
description: Change your password, display preferences, and API key.
sidebar:
  order: 2
---

Open your username at the bottom of the sidebar. **My account** shows your username, role, and API access. **Settings** controls how dates and interface text appear for your account.

## Change your password

From **My account**, choose **Change password**. Enter your current password, then the new password twice. Successful changes return you to your account and revoke your other sessions. Initial account setup uses a separate required password-change screen.

New passwords must contain at least eight UTF-8 bytes and at most 1,024 bytes; confirmation must match, and the current password cannot be reused. Byte length and character count differ for some languages.

If you cannot sign in, ask another authorized administrator to reset your account. There is no documented email-based self-service recovery flow. [Account administration](../administration/accounts-and-permissions.md) explains the protections on administrative changes.

## Choose language and time presentation

In **Settings**, choose English or German interface text, a date/time formatting locale, and a time zone independently. For example, you can use English interface text with German date formatting and a Berlin time zone. Reset an override to follow the browser again. These choices follow your account across devices.

The preview shows your selected presentation. Save to apply it. Failed saves keep your edits for retry; refreshing a dirty form does not silently replace your choices. Display preferences do not move existing campaign schedules. See [scheduling](campaigns/scheduling.md) for local-time input behavior.

## Use or replace your API key

On **My account**, reveal or copy the API key only when you need it. Copying a hidden key keeps it masked on screen. Replacing the key immediately invalidates the previous one, so update integrations that use it.

The key acts with your account's permissions. Keep it out of shared screenshots, URLs, and logs. See [API authentication](../api/authentication.md) for request examples.

## Sign out

Choose **Sign out** from the account menu. Unsaved work triggers a discard prompt. If sign-out fails, the session and draft remain so you can retry. Signing out ends this login; independently authenticated browsers remain signed in unless another account change revokes them.
