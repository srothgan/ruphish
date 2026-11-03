---
title: Troubleshooting
description: Find the next step when resources, campaigns, or results do not behave as expected.
sidebar:
  order: 51
---

Start with the message shown in Ruphish and the last action you took. Where an error includes an application code, keep that code alongside the explanation; do not include passwords or API keys when asking for help.

| Problem | Check first |
| --- | --- |
| A resource or action is unavailable | Confirm your account's permissions. Reporting mailboxes and webhooks also need to be enabled in company settings. |
| A resource name is already used | Names are shared within each resource type. Open the existing resource or choose a distinct name. |
| An import is missing after a reload | Imports are drafts until **Create** succeeds. Import again if the draft was lost. |
| A selected campaign resource is unavailable | It may have been deleted or changed. Replace the selection and refresh campaign review. |
| A domain cannot be selected | Check enablement and the installation's verification policy in [Domains](resources/domains.md). |
| A campaign link works on your computer but not a phone | A local `127.0.0.1` address points to the device opening it. Use a reachable [hosted domain](resources/domains.md#choose-the-address). |
| A connection check works but sending fails | Check the provider's sender and recipient restrictions with a [test email](resources/sending-profiles.md#check-the-connection-and-send-a-test). |
| A campaign has not started sending | Confirm it was launched, inspect its due time, and check [Sending](campaigns/monitor-sending.md). |
| A send is unconfirmed | Check provider evidence before [resending or marking it sent](campaigns/monitor-sending.md#resolve-an-unconfirmed-send). |
| An imported page looks different | Inspect preview and the page checks; review [supported content and assets](resources/landing-pages.md). |
| A report does not appear | Check the mailbox history, preserved campaign link, and campaign status in [Reporting mailboxes](resources/reporting-mailboxes.md). |
| Counts differ between views | Compare filters, observed events, repeat counts, and progression in [Metrics](results/understand-the-metrics.md). |
| An export is too large | Narrow the results filters before [exporting](results/export-results.md). |

## A save conflicts with another edit

Ruphish protects shared work from being overwritten by stale forms. Preserve your unsaved text, open the current item in another tab, and compare the changes.

Do not repeatedly submit the same stale version. Reload only after you have kept anything you need, then apply your changes to the current version.

## A save or launch has no clear outcome

A lost response does not prove that the server rejected the action. Keep the form open and use the provided retry action where available.

For a campaign launch, also inspect **Campaigns** before creating another campaign. For imported or starter content, inspect the resource list before adding it again.

## A refresh fails

The page can retain the last successfully loaded data during a temporary failure. Use the error and retry controls before relying on that data as current.

If the failure affects the whole installation, ask the person hosting Ruphish to investigate using [administration troubleshooting](../administration/hosting-and-maintenance/troubleshoot-the-installation.md). Provide the page, action, time, and error code, with sensitive values removed.
