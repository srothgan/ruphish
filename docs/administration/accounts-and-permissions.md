---
title: Manage accounts and permissions
description: Administer access while preserving ownership and a usable administrator account.
sidebar:
  order: 6
---

Open **Administration → Users** to create, edit, lock, or delete application accounts. Choose a role deliberately; effective permissions are read from persisted role mappings rather than guessed from the displayed role name.

| Capability | Permission/boundary |
| --- | --- |
| Read owned campaign resources and reports | `view_objects`, plus ownership checks |
| Create/change owned resources and campaigns | `modify_objects`, plus operation-specific checks |
| Read owned webhook configurations and history | `view_objects` |
| Create/change owned webhooks and reporting mailboxes | `modify_objects`; administrator feature switches must allow creation, saves, and network actions |
| Manage other accounts and system administration | `modify_system`, with mutation permissions where required |
| Change your own password/display preferences | Dedicated browser-session workflow; not an arbitrary public API operation |
| Access another account's campaigns as that account | Explicit administrator impersonation, with actor/effective identity retained |

Custom persisted roles can combine permissions; the UI's role labels are not a substitute for this boundary. Locked accounts and accounts with unfinished password setup cannot use API keys.

## Create and recover accounts

Choose **Create user**, provide the required identity/role/password information, and save. Existing-account edits open a separate form. Failed saves keep non-secret inputs and clear the entered password.

For an account that cannot sign in, an authorized administrator can reset another account's password. Users change their own password through the dedicated current-password workflow. Existing Go bcrypt hashes remain usable without automatic rehashing on login; new/changed passwords use Argon2id. This affects [rollback](../migration/verification-and-rollback.md).

## Lock, delete, and impersonate

Locking or changing account privileges invalidates stale access through current account state. Password changes revoke other sessions. A request already authorized can finish; revocation is not a transaction cancelling every external action.

You cannot delete your own account. Deleting, locking, or demoting the last unlocked administrator is also rejected. Account deletion removes owned records transactionally while preserving other owners' data. Successful deletions record actor and target identifiers; this is not a claim of a comprehensive audit log for every UI action.

**View as** enters explicit impersonation and displays a banner. Account and credential management are blocked while impersonating. Exit impersonation to return to the original identity.

[Company settings](settings.md) control domain verification, reporting mailboxes, webhooks, and confirmed closure of active campaigns. Listener, database, and network configuration remains in [configuration](configuration.md).
