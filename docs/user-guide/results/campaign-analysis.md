---
title: Analyze a campaign
description: Filter observations and inspect a recipient's timeline.
sidebar:
  order: 4
---

Open **Analysis** and select a campaign, or choose **View analysis** from its campaign page. The analysis is scoped to that campaign.

Use the recipient selector, event/source filters, and time range to narrow the question. Select up to 200 recipient identities, or leave the selector empty for everyone. Local time input follows your display zone; the underlying filters use UTC instants.

The recipient table remains below the chart. Choose a recipient's **Details** action to inspect their events and chart. Toggle series in the chart legend, then hover, focus, or select a time column for interval details. Quiet intervals within the observed range have zero counts.

Select a column heading to sort ascending; select it again to reverse the order. The arrow and accessible header state show the active direction. Recipient email, name, delivery state and event counts can be sorted; recipient details also support sorting by time, event, source and credential presence. Changing order returns to the first page. The selected order remains in the page URL for bookmarks, recipient drilldown and browser navigation, and downloads use that order across all matching rows.

Filters apply before totals and pagination. Moving between table pages does not change the full filtered aggregate. Refresh failures retain the last confirmed result; use the visible error/retry before drawing conclusions from stale data.

Use [exports](exports.md) to download all matching rows, and [metric definitions](metrics.md) to distinguish recorded events from summary progression.
