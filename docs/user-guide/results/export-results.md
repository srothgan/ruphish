---
title: Export results
description: Download filtered recipient activity and individual events as CSV or Excel.
sidebar:
  order: 34
---

Use exports to analyze results elsewhere or share a record of the campaign's current observations.

## Download campaign results

1. Open the campaign's **Results** tab.
2. Set the recipient, event, source, and time filters you need.
3. Choose **Apply** and wait for the results to load.
4. Choose **Recipient activity CSV**, **Individual events CSV**, or **Export Excel**.
5. Check the downloaded file before sharing it.

Downloads include all matching rows, not just the visible table page. Recipient downloads follow the selected table order.

| Format | Use it for |
| --- | --- |
| Recipient activity CSV | One row per recipient, including identity, delivery state, and counts of recorded events. |
| Individual events CSV | One row per event, including time, source, edited status, and available presence/client observations. |
| Excel | A workbook with Summary, Recipients, and Events sheets. |

The individual **Recipient activity** page also offers an export for that person's history.

## Understand the scope

Event filters select recipients with any matching event, then retain their events within the same time and source scope. The start time is included; the end time is excluded.

Recipient activity exports include repeat-event counts. Three clicks by one recipient count as three click events in that row, although the campaign Results card counts that recipient once. Do not sum repeat counts to calculate participant totals.

Exports contain current event values and omit deleted events. The source identifies where an event came from; the edited flag identifies a later change.

## Interpret dates and empty values

Exported timestamps use UTC. Recipient send and modification dates describe stored recipient records independently of the selected event window.

A send date can refer to scheduled or retry work, so do not treat the date alone as evidence of acceptance. Check the delivery state and recorded sent events.

Missing dates remain blank. When automatic open tracking is disabled, recipient open-count cells show **Not tracked**.

## Handle limits and sharing

An export sheet is limited to 100,000 rows, and the input snapshot is limited to 32 MiB. Narrow the filters if an export exceeds a limit.

CSV protects cells that could be interpreted as spreadsheet formulas; Excel writes text values as text. Keep that protection when opening files in a spreadsheet.

Exports contain recipient identities and activity even though they exclude entered form values. Share them with people who should have access, and manage downloaded copies separately from campaign deletion.

For exact column definitions and automated downloads, see the [export API](../../api/results-and-exports.md).
