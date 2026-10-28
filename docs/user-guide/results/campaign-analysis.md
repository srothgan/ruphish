---
title: Inspect campaign results
description: Filter observations and inspect a recipient's timeline.
sidebar:
  order: 4
---

Open **Campaigns** and select a launched campaign. **Results** opens by default, during execution and after completion. The shared header also offers **Setup**, **Delivery**, and **Connections**.

Use the recipient selector, event/source filters, and time range to narrow the question. Select up to 200 recipient identities, or leave the selector empty for everyone. Local time input follows your display zone; the underlying filters use UTC instants.

The recipient table remains below the chart. Expand a recipient to inspect individual observations and delivery attempts. **Details** opens the same recipient detail at a direct location; returning preserves the campaign filters, order, and page. Toggle series in the chart legend, then hover, focus, or select a time column for interval details. Quiet intervals within the observed range have zero counts.

Select a column heading to sort ascending; select it again to reverse the order. The highlighted triangle and accessible header state show the active direction. Unsorted indicators appear when the header is hovered or its button receives keyboard focus. Recipient email, name, delivery state and event counts can be sorted; Changing order returns to the first page. The selected order remains in the page URL for bookmarks, recipient drilldown and browser navigation, and downloads use that order across all matching rows.

Filters apply before totals and pagination. Moving between table pages does not change the full filtered aggregate. Visible Results pages refresh automatically about five seconds after each completed read, including completed campaigns. Filters, ordering, page and recipient selection stay in place. Hidden pages pause periodic refresh; leaving the page stops its timer. Temporary failures retain the last confirmed result and increase the retry delay up to 160 seconds; use the visible error/retry before drawing conclusions from stale data. Access-denied or missing-record failures remove stale results and stop periodic retries.

Use [exports](exports.md) to download all matching rows, and [metric definitions](metrics.md) to distinguish recorded events from summary progression.

Use [event corrections](correcting-events.md) in the expanded table to edit, exclude, add or restore individual events. Charts and counts use effective events; original evidence remains inspectable.
