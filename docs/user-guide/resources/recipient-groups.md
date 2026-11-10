---
title: Recipient groups
description: Add recipients, import CSV files, and reuse audiences across campaigns.
sidebar:
  order: 11
---

A recipient group is a reusable list of people. Each recipient has an email address and optional first name, last name, and position. These details also supply values for [personalized content](../personalize-content.md).

Create groups that make campaign selection easy, such as a department or a training cohort. One campaign can include several groups.

## Add recipients

1. Open **Resources → Recipient groups** and choose **Create**.
2. Enter a recognizable group name.
3. Choose **Add recipient** and enter the person's email address. Add their name and position if you want to use them in messages or results.
4. Repeat for the remaining recipients, then choose **Create**.

Open the saved group and check its recipients before using it. To change the list later, choose **Edit**, add or remove recipients, and save.

The create and edit forms load existing recipients in ascending email order once. Each new recipient appears at the top and stays there while you edit. These forms have no sorting controls and do not reorder rows while typing, leaving fields, or saving. The saved group's read-only view remains sortable.

## Import a CSV file

1. From **Recipient groups**, choose **Import**.
2. Choose **Download recipient CSV template**, or prepare a UTF-8 CSV file with the columns below.
3. Replace the example recipients with your intended audience.
4. Select your file or paste its contents.
5. Review the accepted recipients, duplicate omissions, and rejected rows. Download the error report if you need to correct the source file.
6. Acknowledge any reported omissions or errors, continue to the create form, and save the group.

```csv
First Name,Last Name,Email,Position
Alex,Rivera,alex@example.test,Operations
Sam,Lee,sam@example.test,Support
```

Only the email column is required. Missing optional columns become empty values. Column matching ignores capitalization, spaces, and underscores, so `First Name` and `first_name` both work. If your column names differ, expand **Advanced → Column mapping** and enter the names used in your file.

Import review keeps the first accepted occurrence of an email address, ignoring differences in capitalization. Invalid addresses, malformed rows, or invalid encoding appear in the error report. Correcting the file and importing it again is usually easier than repairing a large list manually.

CSV files can contain up to 100,000 records and 1,024 columns, with an 8 MiB file limit. Imports remain unsaved until **Create** succeeds; closing or reloading the tab loses the import draft.

## Use overlapping groups

Ruphish automatically removes duplicate email addresses from the selected groups. If duplicate entries have different names or positions, it keeps the details from the first occurrence and shows the differences in review. You do not need to remove overlapping recipients manually.

CSV import and campaign audience review are separate checks. In particular, CSV review ignores email capitalization when finding duplicates; campaign selection compares exact addresses. Use consistent email spelling in your source lists.

## Change or remove a group

Editing one group does not update the same person in another group. It also does not change the recipients of a campaign already launched.

Use **Duplicate** for an independent copy, or **Export JSON** to transfer a group to another installation. Delete a group only when you no longer need it; deletion is blocked while an unfinished campaign references it.
