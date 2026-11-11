---
title: Sending profiles
description: Connect a dedicated mail account and check that it can send campaign messages.
sidebar:
  order: 12
---

A sending profile connects Ruphish to a mail account through SMTP. Use a dedicated account for your campaigns so you can manage its sending limits, credentials, and delivery records separately from personal mail.

Before you begin, get the SMTP hostname, port, username, password or supported app password, and allowed sender address from your mail provider. Use the provider's SMTP settings rather than its webmail address.

## Connect your account

1. Open **Resources → Sending profiles** and choose **Create**.
2. Enter a name that identifies the account.
3. In **SMTP host and port**, enter the provider's hostname and port, for example `smtp.example.com:587`. Replace this example with your provider's actual settings.
4. Enter the **Sender address** allowed for that account.
5. Enter the username and password if the service requires authentication.
6. Leave **Ignore SMTP certificate errors** off.
7. Choose **Create**.

Port 465 uses an encrypted connection from the start. Other ports require STARTTLS before authentication or message submission. An explicitly allowed loopback relay can still operate without TLS for local testing; allowing an internal network does not disable TLS for remote servers. Ruphish does not provide an OAuth account-connection flow.

IPv4 and IPv6 are selected automatically. Ruphish checks the resolved destinations before connecting, tries alternative approved addresses when a connection is unavailable, and authenticates and submits through one selected connection. You do not need to choose an address family. Keep the provider's hostname in the profile so TLS can verify its identity.

Your administrator can require a successful profile test before new campaigns launch. This checks SMTP submission, not ownership of the sender address or domain. Complete any sender-ownership verification separately in your provider's service.

## Check the connection and send a test

1. Open the saved profile's view. Testing is available here, without entering **Edit**.
2. Open **Tests** and choose **Test connection** to check connectivity and any configured authentication immediately, without sending a message.
3. Open **Tests** and choose **Send test email** to reveal its separate recipient card above the profile settings.
4. Enter an inbox you control and choose **Send test email**. Ruphish sends a fixed plain-text message; no template, landing page, URL, or personal details are required.

A successful connection check does not test whether the provider accepts your message or sender. A test email checks submission, but the final check is finding it in the recipient inbox.

The profile view shows separate connection and email-submission results with timestamps, plus the latest ten completed tests. History survives a restart and includes the operator, recipient when applicable, outcome, duration, and diagnostic details. One operational test can run per profile at a time.

Changing sending settings or credentials marks earlier evidence as **Settings changed** and requires a new test when company policy demands one. Renaming the profile or saving unchanged settings preserves the evidence. Duplicating a profile starts with no test history.

When **Require successful sending-profile test** is enabled in [company settings](../../administration/team-administration/company-settings.md#require-a-successful-sending-profile-test), a current-configuration email submission must have been accepted before a new campaign can launch. Connection-only success does not qualify. A later failed, interrupted, or unknown test revokes readiness until another profile email test succeeds. Tests do not expire automatically, and this policy does not pause campaigns already launched.

When testing from an [email template](email-templates.md), select this profile to check your actual content. If the button says **Save and send test**, it saves your changes before sending; a sending failure does not undo the save.

If the request response is lost, use **Check result** to reconcile the same operation without sending another email. If the stored submission outcome is unknown, check the inbox and provider's logs before deliberately starting another send. The first attempt may already have been accepted. Profile tests create no campaign events or public preview links.

## Choose the sender recipients see

The profile's **Sender address** supplies the address used when submitting mail to the server and the default visible From address. An email template can override the visible From address.

For example, a template may show a different display name while the profile still submits mail through the dedicated sending account. The provider must permit the addresses you use. Check both sender values in campaign review and inspect a test message.

Use **Additional headers** only when your mail setup needs them. Add each header as a name/value pair and test the resulting message.

## Update credentials or connection settings

Open **Edit** to change the profile. A blank password field keeps the saved password. Enter a new value to replace it, or select **Clear the saved password** to remove it.

Passwords are not included in profile exports or copies. Enter the password again when creating a profile from an import or duplicate.

Connection, password, and certificate settings can help recover sending for existing campaigns. Changing the sender or template content does not rewrite messages in campaigns already launched. After fixing a connection problem, inspect [campaign sending](../campaigns/monitor-sending.md) and retry the affected deliveries where needed.

## Resolve a failed test

| What you see | What to check |
| --- | --- |
| Connection or destination rejected | Confirm the hostname and port. For an internal mail server, ask the host operator to check [outbound access](../../administration/hosting-and-maintenance/connect-to-internal-services.md). |
| Certificate error | Check the hostname, port, certificate, and trusted roots. Keep certificate checking enabled for your provider. |
| Authentication failed | Check the account's credentials and whether SMTP access or an app password must be enabled with the provider. |
| Sender rejected | Check that the provider allows the sender address and any template override. |
| Test accepted, but no message in the inbox | Check spam filtering, the sending mailbox for a bounce, and the provider's delivery logs. See [delivery failures after sending](../campaigns/monitor-sending.md#delivery-failures-after-sending). |

A sending profile cannot be deleted while an unfinished campaign uses it.
