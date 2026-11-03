---
title: Shared access and permissions
description: Understand the shared workspace, account roles, and resource attribution.
sidebar:
  order: 11
---

One Ruphish installation is one shared workspace. Colleagues can work with the same resources, campaigns, recipients, and results, including items created by someone else.

## Choose an account role

New installations provide two roles:

| Role | What the account can do |
| --- | --- |
| **User** | Create and manage shared resources and campaigns, view results, and export data. Manage reporting mailboxes and webhooks when those features are enabled. |
| **Administrator** | Everything a User can do, plus manage accounts, company settings, and the starter-template catalog. |

Choose **User** for someone who runs campaigns. Choose **Administrator** for someone who also manages access or company-wide policies. There is no separate read-only role or team hierarchy in the account form.

On migrated or customized installations, existing role permissions can differ from these defaults. Ruphish checks the stored permissions: `view_objects` allows reading shared data, `modify_objects` allows changing it, and `modify_system` allows system administration. See the [accounts API](../../api/users-and-api-keys.md) for automation.

## What is shared

Resources, campaigns, recipient details, results, exports, integration configurations, and integration history are shared with accounts that have the relevant permissions. This includes saved sending and reporting accounts: colleagues can use them without the application revealing their saved passwords.

Passwords, sessions, API keys, and personal display preferences belong to individual accounts. An account with unfinished password setup or a locked account cannot use its API key.

Use separate installations when groups must not access each other's campaign data. Account roles do not create separate workspaces within an installation.

## Work on shared resources

Resource details show who created the resource and who last edited it. A successful save updates the last editor and time. Imported records may have no last-editor information.

If someone saves a resource while you are editing it, reload their version and review the changes before saving again. A failed or rejected save does not replace their work.

Choose distinct names for recipient groups, sending profiles, email templates, landing pages, learning pages, and reporting mailboxes. Each of these resource types requires unique names across the workspace. Campaign, domain, and webhook display names may repeat. If imported resources have duplicate names, open the specific item rather than relying on its name alone.

Changing a shared resource can affect future campaigns. A launched campaign keeps its saved content and audience, while its sending connection and domain eligibility remain live. See [creating a campaign](../../user-guide/campaigns/create.md) for what stays fixed after launch.

## When someone leaves

Locking or deleting an account removes that person's access. It does not stop their campaigns, disable integrations, or delete shared work. Review those items separately if they should stop. Follow [Manage accounts](accounts.md) for the account steps and [Close active campaigns](close-campaigns.md) if campaigns must end.
