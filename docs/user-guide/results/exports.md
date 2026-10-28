---
title: Export campaign results
description: Download filtered recipient or event records as CSV or Excel.
sidebar:
  order: 5
---

Set the campaign Results filters first, then choose **Recipient activity CSV**, **Individual events CSV**, or **Export Excel**. Exports contain all matching rows, not only the visible page. Selected event types choose recipients who have any of those events within the date/source scope; exports retain all event types for those recipients within that same scope. The start time is inclusive and the end time is exclusive.

| Download | Contents |
| --- | --- |
| Recipient activity CSV | Recipient identity, separate first/last names, position, stored send/modification dates, and delivery/observed outcome counts |
| Individual events CSV | Effective events, their corrected time, source, edited flag, and attributable recipient |
| Excel | Summary, Recipients, and Events sheets using the same filters |

Exports keep machine-readable timestamps; the minute precision of a chart label does not reduce stored/exported precision. Presence metadata is a boolean where recorded. Captured participant values, request IPs, and location data are not export fields.

Recipient dates are exported in UTC from the stored campaign results, independently of the selected event window. Missing, zero or invalid historical dates are blank. A send date can describe scheduled or retry work as well as an accepted send; check the delivery state and observed acceptance count before interpreting it as delivery evidence. Excel includes the same recipient columns as CSV.

Each sheet is limited to 100,000 rows and the snapshot input to 32 MiB. Narrow the filters when the export exceeds a limit. Spreadsheet-sensitive CSV cells are neutralized, and Excel text is written as text. If your downstream process compares literal cell values, account for that CSV protection rather than stripping it automatically in a spreadsheet.

See [the export API](../../api/analysis-and-exports.md) for exact columns, filters, format/scope options, and automated downloads.

Event exports contain current values and omit deleted events. The origin identifies how an event was created; the edited flag identifies a subsequent manual change.

Campaign summary cards and the campaign chart count participants once per action. Recipient activity exports deliberately retain repeat counts, and individual event exports retain one row per event. Do not sum repeat counts to calculate participant totals.
