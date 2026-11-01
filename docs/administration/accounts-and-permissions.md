---
title: Manage accounts and permissions
description: Administer a shared workspace while preserving a usable administrator account.
sidebar:
  order: 6
---

Open **Administration → Users** to create, edit, lock, or delete application accounts. Choose a role deliberately; effective permissions are read from persisted role mappings rather than guessed from the displayed role name.

| Capability | Permission/boundary |
| --- | --- |
| Read shared resources, recipient identities and campaign reports | `view_objects` |
| Create/change shared resources and campaigns | `modify_objects`, plus operation-specific checks |
| Read shared webhook configurations and history | `view_objects` |
| Create/change shared webhooks and reporting mailboxes | `modify_objects`; administrator feature switches must allow creation, saves, and network actions |
| Manage other accounts and system administration | `modify_system`, with mutation permissions where required |
| Change your own password/display preferences | Dedicated browser-session workflow; not an arbitrary public API operation |
| Use shared campaigns created by colleagues | Normal resource permissions; no impersonation required |

Custom persisted roles can combine permissions; the UI's role labels are not a substitute for this boundary. Locked accounts and accounts with unfinished password setup cannot use API keys.

## Create and recover accounts

Choose **Create user**, provide the required identity/role/password information, and save. Existing-account edits open a separate form. Failed saves keep non-secret inputs and clear the entered password.

For an account that cannot sign in, an authorized administrator can reset another account's password. Users change their own password through the dedicated current-password workflow. Existing Go bcrypt hashes remain usable without automatic rehashing on login; new/changed passwords use Argon2id. This affects [rollback](../migration/verification-and-rollback.md).

## Lock, delete, and impersonate

Locking or changing account privileges invalidates stale access through current account state. Password changes revoke other sessions. Database writes recheck authorization inside their transaction. Already committed campaigns and integrations continue after creator locking or deletion; use their own lifecycle controls or global switches to stop them. External actions already in flight cannot be recalled.

You cannot delete your own account. Deleting, locking, or demoting the last unlocked administrator is also rejected. Account deletion revokes access and removes personal preferences and preview requests. Shared resources, campaigns, credentials, queued work, integration progress and results remain intact. Creator and last-editor identities remain attached to a non-authenticating deleted-account record; a later account with the same username has a different identity. Successful deletions record actor and target identifiers; this is not a claim of a comprehensive audit log for every UI action.

**View as** enters explicit impersonation and displays a banner. Account and credential management are blocked while impersonating. Exit impersonation to return to the original identity.

[Company settings](settings.md) control domain verification, reporting mailboxes, webhooks, and confirmed closure of active campaigns. Listener, database, and network configuration remains in [configuration](configuration.md).

## Shared resources and attribution

One installation is one company workspace. All authenticated accounts with the relevant permissions can list, read, select, edit, copy and delete resources and campaigns created by colleagues. This includes recipient identities, results, exports, integration configuration and delivery history. Account credentials, sessions and preferences remain personal; account administration and global settings remain administrator-controlled. There is no ownership-mode switch, team hierarchy or separate viewer/editor role in this release.

Resource details show the creator and the last editor with a timestamp. These values are read-only: a successful save records the authenticated editor automatically. Background jobs, previews, failed saves and stale-edit rejections do not rewrite edit attribution. Imported records without reliable last-editor evidence show **-**. When another user changes a resource, reload and review their saved version before retrying your edit.

Recipient-group, email-template, landing-page, learning-page, sending-profile and reporting-mailbox names must be unique within their family across the installation; campaign, domain and webhook display names can repeat. Imported duplicate names retain their IDs and data. Select them by ID; ambiguous name-based requests fail. Duplicate historical domain origins retain existing campaign references, but new launches require exactly one matching registration to be enabled. Disable unused duplicates to resolve selection without deleting historical campaigns. [Launched campaign content stays frozen](../user-guide/campaigns/launch.md#what-stays-fixed-after-launch), while SMTP transport and domain eligibility remain live.
