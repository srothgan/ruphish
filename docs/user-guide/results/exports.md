---
title: Export campaign results
description: Download filtered recipient or event records as CSV or Excel.
sidebar:
  order: 5
---

Set the campaign-analysis filters first, then choose **Recipients CSV**, **Events CSV**, or **Export Excel**. Exports contain all matching rows, not only the visible page.

| Download | Contents |
| --- | --- |
| Recipients CSV | Recipient identity and delivery/observed outcome counts |
| Events CSV | Individual observations, their time/source, and attributable recipient |
| Excel | Summary, Recipients, and Events sheets using the same filters |

Exports keep machine-readable timestamps; the minute precision of a chart label does not reduce stored/exported precision. Presence metadata is a boolean where recorded. Captured participant values, request IPs, and location data are not export fields.

Each sheet is limited to 100,000 rows and the snapshot input to 32 MiB. Narrow the filters when the export exceeds a limit. Spreadsheet-sensitive CSV cells are neutralized, and Excel text is written as text. If your downstream process compares literal cell values, account for that CSV protection rather than stripping it automatically in a spreadsheet.

See [the export API](../../api/analysis-and-exports.md) for exact columns, filters, format/scope options, and automated downloads.
