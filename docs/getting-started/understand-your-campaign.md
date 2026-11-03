---
title: Understand your campaign
description: Understand how your campaign's audience, content, sending account, and results fit together.
sidebar:
  order: 4
---

In [your first campaign](run-your-first-campaign.md), you brought together an audience, a message, a destination, and a way to send mail. These are reusable **resources**. A campaign combines them for one exercise and keeps its own recipient history.

## How the resources fit together

| Resource | Its job | In your first campaign |
| --- | --- | --- |
| Group | Lists the recipients and details such as their names | Your own inbox in `First campaign audience` |
| Email template | Defines the subject, message, and optional attachments | The Google Drive document-sharing invitation |
| Landing page | Provides the page opened by a recipient's campaign link | The simulated Google Drive document page |
| Learning page | Provides an explanation after submission | The SLAM guidance |
| Sending profile | Connects Ruphish to your mail service and sets the default sender address | Your dedicated sending account in `Training mail account` |
| Domain | Sets the base web address used for campaign links | `http://127.0.0.1:8081` in `Training campaign pages` |

You sign in to Ruphish to manage campaigns. Recipients follow links to your campaign pages at a separate web address. They must be able to open those pages from the devices where they read the email.

One Ruphish installation is a shared company workspace. Depending on their permissions, users can view and edit resources and campaigns created by colleagues. Creator and editor details show who prepared them. Recipients are the people receiving the campaign emails; they don't need user accounts. See [accounts and permissions](../administration/team-administration/shared-access-and-permissions.md) for who can do what.

## From draft to campaign

A **draft** saves your selections while you prepare. You can return to it, change content, and review the audience. Saving it never starts delivery, even if a proposed launch time passes.

**Launching** starts the campaign, or schedules it to start at the time you chose. During review, Ruphish combines the selected groups into one recipient list and removes duplicate addresses. A person listed in several groups receives one campaign message.

At launch, Ruphish saves the recipient list, content, and tracking choices for that campaign. Editing an email template afterward affects future campaigns. You can still update the sending profile's connection settings to fix a delivery problem. See [what stays fixed after launch](../user-guide/campaigns/create-a-campaign.md#understand-what-launch-preserves) for the details.

**Completing** a campaign stops remaining deliveries and new recipient activity while keeping its history. **Deleting** it removes that history. You can't delete resources that an unfinished campaign still uses.

## What the results tell you

Delivery and participant interactions answer different questions:

- **SMTP acceptance** means the mail server accepted the message. It doesn't prove that the message reached the inbox or that a person read it.
- **An open** means the tracking image was requested, when open tracking is enabled. Image proxies and automated clients can cause that request.
- **A click** means a campaign link was requested. It may come from a person or an automated scanner.
- **A submission** records that the form was submitted and whether the password field contained anything. Ruphish doesn't store the usernames, passwords, or other values entered in the form.
- **A report** records that the message was reported as phishing, separately from clicks and submissions.

The timeline shows individual recorded events. Summary counts also reflect how far recipients progressed: for example, a click can count toward opens even when no tracking image was loaded. See [how metrics are counted](../user-guide/results/understand-the-metrics.md) when interpreting results or comparing exports.

If a delivery has an **unknown outcome**, the SMTP service may have accepted the message before a connection failed. Check the receiving system before choosing to resend; see [delivery recovery](../user-guide/campaigns/monitor-sending.md).

## Prepare your next exercise

Reuse your resources when they still fit, or create new ones for a different audience and lesson. Start with [email templates](../user-guide/resources/email-templates.md), [landing pages](../user-guide/resources/landing-pages.md), and [groups](../user-guide/resources/recipient-groups.md). Check that the sending account and campaign address still suit the new campaign, then review and launch it.
