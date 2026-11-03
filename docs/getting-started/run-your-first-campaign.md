---
title: Run your first campaign
description: Connect a sending account, use starter content, and follow a campaign through your own inbox.
sidebar:
  order: 3
---

Your first campaign sends a document-sharing invitation to an inbox you control. You'll use ready-made content from the **Starter Template Repository**, open the campaign link on the computer running Ruphish, and inspect the results. You don't need a public domain for this local tryout.

## Before you begin

You'll need:

- A running Ruphish installation and an account with permission to create resources and campaigns. The administrator created during [installation](install-ruphish.mdx) can also access the Starter Template Repository.
- An SMTP service you can send through. Use a dedicated sending account for your exercises, with its SMTP hostname, port, username, password, and permitted From address.
- An inbox you control and can check.
- Access to your inbox on the computer running Ruphish, so you can open the local campaign link there.

SMTP is how Ruphish connects to your mail provider to send email. Your provider may require you to enable SMTP access, create an app password, or verify a sender address or domain. Follow its setup instructions and use the SMTP login details it provides.

## 1. Connect your sending account

A **sending profile** stores the settings Ruphish uses to send email through your account. Open **Resources → Sending profiles → Create** and enter:

| Field | What to enter |
| --- | --- |
| Name | `Training mail account`, or another name you'll recognize |
| SMTP host and port | Your provider's SMTP hostname and port, such as `smtp.example.com:587`; replace this example with the provider's actual settings |
| From address | An address your account is allowed to send from, optionally with a display name such as `Training <training@example.com>` |
| Username | The SMTP username supplied by your provider, often the full sending-account email address |
| Password | The SMTP password or app password required by your provider |

Keep **Ignore SMTP certificate errors** off and use the host and port specified by your provider. The [sending-profile guide](../user-guide/resources/sending-profiles.md) explains connection security if you need more detail.

Save the profile. Open **Send test** and choose **Test connection only**. This checks whether Ruphish can connect and sign in to your mail service, without sending a message.

If sign-in fails, check the SMTP username and password and whether SMTP access is enabled for the account. For a connection or certificate error, check the hostname, port, and provider's instructions. If you use an internal company mail server, the person managing Ruphish may need to [allow connections to it](../administration/hosting-and-maintenance/connect-to-internal-services.md).

## 2. Send yourself a test message

In **Send test**, enter your own inbox address under **Recipient email**. Leave the optional public test URL and landing-page selection empty, then choose **Send test email**. Ruphish can send a default test message before you've created campaign content.

Check your inbox and junk folder. Confirm that the message arrived and shows the expected sender address. If Ruphish says the mail server accepted it but it doesn't arrive, check your provider's delivery logs or filtering before retrying.

## 3. Add starter content

Open **Company settings → Starter Template Repository**. You'll add an email, a landing page, and a learning page. Each is saved as a copy you can edit; adding them doesn't send any email.

1. Under **Email templates**, select **Google Drive - Document sharing invitation (EN)**. Use **Preview** to inspect the message.
2. Under **Landing pages**, select **Google Drive - Shared document**. Leave **Prefill recipient email** and **Use company logo** off for this first campaign.
3. Under **Learning pages**, select **SLAM (EN)**. This is the explanation you'll show after a test submission. Leave **Use company logo** off.
4. Choose **Add selected** and check that all three additions succeed.

The email invites its recipient to open “Project review notes.” Its campaign link leads to your selected landing page. The page simulates a document-sharing sign-in; it doesn't retrieve a Google document or validate an account.

If you don't have access to the repository, ask an administrator to add these resources. You can edit the copies later or return to [the repository](../user-guide/add-starter-templates.md) for other designs. If an addition is interrupted, check **Resources** before adding it again.

## 4. Create a group containing yourself

A **group** supplies the audience for a campaign. Open **Resources → Groups → Create**, name the group **First campaign audience**, and add one recipient with your name and the inbox address you tested above. Save and confirm that the group contains only that address.

This first campaign sends a real email. Keeping the audience to your own inbox lets you inspect the complete experience before preparing a wider exercise. Recipients don't need Ruphish application accounts.

## 5. Register your campaign address

Open **Resources → Domains → Create**. Name the domain **Training campaign pages**, enter `http://127.0.0.1:8081` in the origin field, leave it enabled, and save. If you changed the campaign-page port during installation, use that port instead of `8081`.

This address opens campaign pages on your own computer. Open the email's link on the same computer where Ruphish is running; it won't work on your phone or another device.

The local address works with domain verification off, which is the default on a fresh installation. If Ruphish is hosted elsewhere, you want to use multiple devices, or your installation requires verification, follow [Domain setup](../user-guide/resources/domains.md) and use that address instead.

## 6. Review and launch

Open **Campaigns → Create campaign** and select:

| Selection | Value |
| --- | --- |
| Name | `My first campaign` |
| Group | `First campaign audience` |
| Email template | `Google Drive - Document sharing invitation (EN)` |
| Landing page | `Google Drive - Shared document` |
| Sending profile | `Training mail account` |
| Domain | `Training campaign pages` |
| Redirect destination | **Learning page**, then `SLAM (EN)` |

Leave the launch schedule empty to send now and keep **Track email opens** enabled. Check the recipient list: it should contain **one recipient—your own address**. Review the personalized email, From address, campaign link, and selected learning page.

Saving a draft only saves your preparation. Choose **Launch campaign** when you're ready to send.

Open your inbox and find the document-sharing invitation. If it doesn't arrive, check the campaign's delivery view and your mail provider's delivery information before launching another campaign. A mail server can accept a message even if it later ends up in junk or isn't delivered to the inbox.

## 7. Follow the link and inspect your results

In the campaign message, choose **Open document**. The landing page should open at your configured campaign address. Choose its document card to reach the simulated sign-in. To try a submission, enter the fictional address `alex@example.test`, continue, and use the dummy password **`tutorial-only`**. Submit the form; the SLAM learning page should appear.

The page records whether you entered something in the password field, without sending the email address or password you typed to Ruphish. Use the made-up details above for this step. See [participant data](../user-guide/results/participant-data.md) for what Ruphish records.

Return to Ruphish, open the campaign's **Results**, and inspect your recipient timeline. You should find the link click and submission. An email-open event appears only when the mail viewer requests the tracking image.

Mail scanners and image proxies can also produce opens and clicks, so you may see activity before you manually open the message. Use [metric definitions](../user-guide/results/understand-the-metrics.md) to interpret those observations.

Download **Recipients CSV** to see the recipient summary, or **Events CSV** to inspect individual observations.

## Finish and prepare your next exercise

Complete the campaign when you're done. You can still review its history, but Ruphish stops recording new recipient activity for it. Deleting a campaign removes its history.

You can reuse the sending profile and starter resources for later campaigns. Before expanding the audience, adapt the message and learning content to your organization and review the recipients and destination again.

## What's next?

- [Understand your campaign](understand-your-campaign.md): learn how resources, delivery, and results fit together.
- [Reporting mailboxes](../user-guide/resources/reporting-mailboxes.md): connect a mailbox to process campaign emails that recipients report as phishing.
- [Webhooks](../user-guide/resources/webhooks.md): send campaign events to other systems.
- [User guide](../user-guide/index.md): explore content, scheduling, and reporting.

Reporting mailboxes and webhooks are optional. An administrator must enable each feature in company settings before you can configure it.
