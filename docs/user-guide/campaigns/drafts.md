---
title: Save and resume a campaign draft
description: Prepare an incomplete campaign without starting delivery.
sidebar:
  order: 2
---

Choose **Campaigns → Create campaign**. Moving between editing steps saves your inputs as a draft. **Save as draft** saves the current step and opens the saved view; an existing draft uses **Save changes**. Watch the save indicator to distinguish unsaved changes from the last confirmed save.

A draft can be incomplete. It has no delivery jobs, public recipient tracking identifiers, campaign events, or analysis results. Its proposed schedule is not active until you explicitly launch.

## Resume your work

Find the draft in Campaigns and choose **Edit**. Editing resumes at its saved step. Missing/deleted resource selections must be replaced before launch. A saved launch time that has passed must be corrected or cleared for immediate delivery.

Entering review checks the current selections and prepares an audience/message preview. That review is not lasting permission to launch: changes to inputs or resources can require review again.

## Recover a failed save

A failed save retains your form values. Resolve the connection or validation problem and retry. If another tab changed the draft, the server rejects the stale revision. Keep a copy of your unsaved values before reloading and reconciling them with the current version.

Deleting a draft removes its saved configuration. It does not send mail or affect an existing execution. After a draft is launched, it links to the campaign and cannot be edited or deleted as an ordinary draft.

Continue with [review and launch](launch.md). API users have a separate [draft contract](../../api/campaign-drafts.md); the legacy campaign list does not include these drafts.
