---
title: Create a campaign
description: Choose your audience and content, save a draft, and review before launch.
sidebar:
  order: 21
---

Create a campaign when your [resources](../resources/index.md) are ready. You need at least one recipient group, an email template, a landing page, a sending profile, and an enabled domain that meets your installation's verification policy.

Check the message and landing page beforehand. A campaign test should go to an inbox you control.

## Choose the audience

1. Open **Campaigns** and choose **Create campaign**.
2. Give the campaign a name that identifies its purpose and audience.
3. Select the recipient groups.
4. Continue to the content step.

Ruphish automatically removes duplicate email addresses from the selected groups. If duplicate entries have different names or positions, it keeps the details from the first occurrence and shows the differences in review. Check the final audience size and retained details; overlapping groups do not require manual cleanup.

Campaign matching uses the exact email address, including capitalization. For example, `Alex@example.com` and `alex@example.com` are treated as different recipients. See [recipient groups](../resources/recipient-groups.md#use-overlapping-groups) for how this differs from CSV import.

## Select content and the campaign address

Choose the email template, landing page, and sending profile. Select the domain that recipients will use to open campaign links.

An optional path or query can make the link fit your content. The base address must remain the selected domain; only the path and query can use recipient personalization.

If a required resource is missing, create it using the provided links, then refresh the available resources. Replace selections that have been deleted or are no longer eligible.

## Choose tracking and the next page

**Track email opens** controls the hidden tracking image in HTML messages. It is enabled by default. Disable it when you do not want this measurement. Plain-text-only email has no tracking image either way.

Clicks and supported submissions are still recorded. Submissions record password presence, not entered usernames or passwords. See [Participant data](../results/participant-data.md).

Under **Redirect destination**, choose:

- **Learning page** to show a saved educational page after a confirmed submission.
- **Custom redirect** to use an HTTP(S) or relative address.
- An empty custom redirect to stay on the landing page.

Review the destination even when it was suggested by the landing page. The campaign keeps the choice made at launch.

## Choose a schedule

Select immediate or scheduled delivery. For a schedule, check the displayed time zone and launch time. You can also set **Initial sending window ends** to spread the initial messages across a period.

See [Schedule a campaign](schedule-a-campaign.md) for timing behavior and limitations.

## Save and resume a draft

Moving between editing steps saves the current input as a draft. **Save as draft** saves the current step and opens the saved view; an existing draft uses **Save changes**.

Check the save indicator before leaving. A draft can be incomplete and has no active sending or participant results.

To continue later, find it in **Campaigns** and choose **Edit**. If its scheduled time has passed, choose a new time or switch to immediate delivery.

If saving fails, your form values remain available. Fix the problem and retry. If someone else changed the draft, preserve your unsaved work and compare it with the current saved version before reloading.

## Review and launch

1. Open **Review** and wait for validation and the rendered sample.
2. Check the audience size and conflicting recipient details.
3. Check the selected resources, campaign address, sender values, and subject.
4. Check the schedule, open-tracking choice, and redirect destination.
5. Send a test email to an inbox you control and inspect it.
6. Choose **Launch campaign** or **Schedule campaign**.

If a selected resource changed after review, refresh the review and check the updated content before launching. Saving a draft or passing review does not start delivery; the launch action does.

After launch, open **Sending** to follow progress. Scheduled messages wait until they are due.

## Understand what launch preserves

Launch saves the audience and authored content, including email, attachments, landing page, learning content, and tracking/redirect choices. Editing the original resources does not update this saved content.

SMTP connection settings and domain availability can still affect delivery. To change the audience or authored content, create a new campaign; **Duplicate** can help start another draft, but review its selections and schedule.

## If launch confirmation is lost

Keep the form open and use its retry action. Ruphish retains the identity of that launch attempt so retrying can recover the same result.

Check the campaign list before starting over. A lost response does not prove that the campaign failed to launch, and creating another campaign could send duplicate messages.
