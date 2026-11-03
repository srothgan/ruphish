---
title: Schedule a campaign
description: Choose a launch time and spread initial sending across a window.
sidebar:
  order: 22
---

Scheduling controls when messages become due for sending. It does not guarantee when they will reach an inbox.

## Send later

1. Open the campaign draft's schedule step.
2. Choose scheduled delivery.
3. Enter the launch date and time.
4. Check the displayed time zone and the resolved time in review.
5. Choose **Schedule campaign**.

The campaign is now launched, with sending waiting until its due time. A saved draft with the same time remains inactive until you launch it.

Changing your account's display time zone later does not move an existing campaign schedule.

## Spread messages across a period

Set **Initial sending window ends** to a time after launch. Ruphish distributes initial due times across the unique campaign audience.

For example, a launch at 09:00 with a window ending at 11:00 spreads the initial sending work across that period. Small audiences and short windows can give several recipients the same due time.

Without an end time, all initial messages become due at launch and are processed as capacity becomes available.

The end of the window is not a delivery deadline. Provider limits, retries, domain checks, or an outage can delay sending beyond it. It also does not complete the campaign.

## Check time zones

Schedule input follows your account's selected time zone or the browser's detected zone. The review shows the resolved time and offset; use these to confirm the intended instant.

Near a daylight-saving change, a repeated local time uses its first occurrence. A local time that does not exist because the clock moves forward is shifted forward. Review such schedules carefully.

You can change display preferences in [Your account](../your-account.md).

## When scheduled sending does not start

Check **Sending** for the affected recipients. Work may be waiting for capacity, a usable domain, or the installation's sending service.

Schedules survive a restart. If sending is disabled for the installation process, ask the person hosting Ruphish to restore it. Recreating the campaign is not the first recovery step.

Use [Monitor sending](monitor-sending.md) to distinguish waiting, failed, and uncertain attempts.
