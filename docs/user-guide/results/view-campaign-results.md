---
title: View campaign results
description: Filter campaign activity and inspect individual recipient histories.
sidebar:
  order: 31
---

Open **Campaigns**, select a launched campaign, and choose **Results**. Completed campaigns open this tab automatically.

The summary cards show how many recipients have recorded opens, clicks, submissions, and reports within the selected filters. Each recipient counts once per action. The participant table shows the same observed activity at the individual level.

## Narrow the results

1. Select recipients if you want to focus on particular people. Leave the selection empty for everyone.
2. Choose event types to find recipients with those activities.
3. Set a source and time range if needed.
4. Choose **Apply** and wait for the new results.

Selecting several event types includes recipients with **any** of those events. It does not require every selected event. Once a recipient matches, all their event types within the same time and source filters remain in scope.

For example, selecting **Clicked** finds recipients with a recorded click in the chosen period. You can still see their submissions and reports in that period.

The start time is included and the end time is excluded. A period from 09:00 to 10:00 includes events at 09:00 and before 10:00. Input follows your display time zone.

Filters apply before totals and pagination. Moving to the next table page does not change the totals. You can select up to 200 individual recipients.

## Read the activity chart

Use **View** to choose:

- **Cumulative:** the number of participants who have recorded each action up to each interval. Quiet intervals keep the preceding total.
- **Per interval:** the number of participants whose first matching action in the selected period falls in that interval. Quiet intervals return to zero.

The chart counts participants, not repeated events or percentages. A recipient clicking five times contributes once to the click series for the selected period.

Use the legend to show or hide event types. Hover, focus, or select an interval for its exact counts. The chart shows its interval duration and display zone.

When email-open tracking is off, open metrics show **Not tracked** and the open series is omitted.

## Inspect one recipient

Select **Recipient activity** in the table to open that person's full campaign history.

Check their delivery status, recorded activity, and individual events. Repeated actions remain separate events. Use **Load more events** when older records are available.

The individual page shows the full history rather than inheriting the campaign's time or event filters. Use the browser's Back action to return to the preceding filtered view.

Click and submission events can show browser, operating-system, and device categories. These describe the request and may differ between events; they do not identify a device with certainty.

Submission indicators distinguish **Entered**, **No entry detected**, and **Not recorded**. Built-in tracking records password presence; username presence normally remains unrecorded. An entered indicator does not mean the credential was valid.

Message context helps identify the related content. Do not treat the currently displayed subject or sender as an archived copy of the exact email that was sent.

## Refresh and continue

Ongoing campaigns refresh while the page is visible. Completed campaigns can be refreshed manually.

If a refresh fails, Ruphish can keep the last confirmed results on screen. Read the error and retry before treating those results as current. Exports become available once the requested filter selection has loaded.

Continue with [metrics](understand-the-metrics.md), [event corrections](correct-events.md), or [exports](export-results.md).
