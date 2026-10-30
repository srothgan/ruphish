---
title: Manage groups and import recipients
description: Create recipient groups, review CSV imports, and handle duplicates and errors.
sidebar:
  order: 3
---

A group is a reusable audience. Each recipient has an email address and optional first name, last name, and position. Those fields are available to [templates](template-reference.md).

## Create or edit a group

Open **Resources → Groups**, choose **Create**, and give the group a name. Add recipients and save. Open a saved group to inspect it; choose **Edit** to make changes. A failed save retains your draft. An edit in one group does not change another group's recipient records.

Names must be unique within your account for newly created or renamed groups. If another tab changes the same resource, a stale save is rejected. Keep your unsaved values while comparing the current saved version in another tab; reloading discards unsaved work.

## Import CSV

Choose **Import** from the group list; CSV is selected by default. Select a file or paste its contents, then continue to the create form to review the recipients. Choose JSON when importing an exported group resource; imports for other resource types start with JSON.

Choose **Download recipient CSV template** in the import dialog to get a UTF-8 file with the supported headers and two synthetic example recipients. Replace the examples with your recipients, then select the file to review it before saving.

```csv
First Name,Last Name,Email,Position
Alex,Rivera,alex@example.test,Operations
Sam,Lee,sam@example.test,Support
```

Use UTF-8; a UTF-8 byte-order mark is accepted. The email column is required. Automatic matching ignores letter case, spaces, and underscores in header names, so `First Name` and `first_name` match. When your file uses different headers, open **Advanced → Column mapping** and enter the corresponding CSV header names. The disclosure starts collapsed and retains your entries when closed. Optional missing columns produce empty values.

Review rejected rows and download the error report if needed. Invalid addresses, encoding errors, and rows whose column count differs from the header are reported. The browser review keeps the first accepted occurrence of each case-insensitive email address and shows duplicate omissions. Acknowledge omissions/errors before creating the group.

Import opens an editable draft; it does not save the resource until **Create** succeeds. The import draft lives in the current tab's authenticated application state. Reloading or closing the tab loses it; import again if it is unavailable.

## Limits and reuse

CSV input is limited to 8 MiB, 100,000 records, and 1,024 columns. One resource is imported at a time. A JSON collection wrapper or array is not a batch-import format.

Selecting several groups for a campaign can create a second kind of duplicate: the same recipient appearing across groups. Campaign review resolves the audience and shows conflicting attributes. Inspect that review before [launching](campaigns/launch.md).

Groups referenced by unfinished campaigns cannot be deleted. Use **Export JSON** from a saved resource when you need a reusable copy. API clients should follow the [groups contract](../api/groups.md); browser CSV review and direct API writes do not have identical duplicate-handling rules.
