---
title: Manage accounts
description: Create accounts, reset passwords, and manage access without removing shared work.
sidebar:
  order: 12
---

Open **Administration → Users** to manage application accounts. You need an Administrator account. Everyone signs in to the same Ruphish website with their own username and password.

## Create an account

1. Choose **Create user**.
2. Enter a username and a temporary password.
3. Choose **User** for campaign work or **Administrator** for account and company administration. See [Shared access and permissions](shared-access-and-permissions.md).
4. Leave **Require password change** selected so the person chooses their own password on first sign-in. Leave **Lock account** clear when the account should be available immediately.
5. Choose **Create user**. The account appears in the user list.
6. Give the person the Ruphish website address, username, and temporary password through your organization's usual secure channel. Ruphish does not email an invitation for you.

The list shows **Password setup** until the person completes the required password change, then **Active**. A locked account shows **Locked**.

## Reset someone's password

1. Open the account from the user list.
2. Enter a new temporary password in **New password (leave empty to keep current)**.
3. Select **Require password change** and choose **Save changes**.
4. Share the temporary password with the account holder. They must choose a new password when they next sign in.

A password change revokes the account's other sessions. Leaving the password field empty preserves the existing password. After a failed save, re-enter the password before trying again; the form clears it.

To change your own password, use the [personal account settings](../../user-guide/your-account.md), which ask for your current password.

## Change a role or temporarily lock an account

Open the account, change **Role** or **Lock account**, and choose **Save changes**. Clear **Lock account** and save to restore access later.

Ruphish applies access checks using the current account state. Locking an account blocks its browser and API access, but already launched campaigns and saved integrations continue. Stop them separately when needed.

Keep at least one usable Administrator account. Ruphish rejects locking, deleting, or demoting the last unlocked administrator.

## Delete an account

Use **Delete** in the user list, review the confirmation, and confirm the removal. You cannot delete the account you are currently using.

Deletion removes access, personal preferences, and preview requests. Shared resources, campaigns, saved integration credentials, delivery work, and results remain. Earlier creator and editor attribution remains associated with the deleted account. Creating another account with the same username does not restore the old identity.

If you may need to restore someone's access later, lock the account instead.

## Help someone with View as

Choose **View as** beside an eligible account to use Ruphish as that person. A banner identifies the impersonated session. Exit impersonation through the banner when you finish.

Account and credential management are blocked while impersonating. You do not need View as to work on a colleague's campaigns or resources; those are already shared.
