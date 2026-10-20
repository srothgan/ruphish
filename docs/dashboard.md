# Dashboard

The authenticated home page at `/` shows analysis metrics, recent campaigns, operational attention items and resource inventory for the signed-in owner. `/campaigns` remains the campaign directory. Administrators see their own campaign data; impersonation follows the selected user's identity.

## Metrics and periods

All time, Last year (a rolling 365 days), Last 30 days and Last 7 days select uniquely identifiable campaign recipients by their first recorded SMTP acceptance. The dashboard defaults to Last 30 days. The server supplies the current UTC time. Rolling windows include their lower boundary and the snapshot time; future events are excluded. Subsequent interactions observed through that snapshot time count once per recipient per campaign. Repeated sends do not move an existing recipient into a newer cohort. The same address in separate campaigns counts separately. Ambiguous historical addresses and events without a matching recipient are excluded from these unique-recipient figures.

All five headline metrics show recipient counts. Opened, clicked, reported and credentials-entered metrics also show their percentage of the selected sent-email count. A zero denominator displays a dash. The period affects only the analysis overview. Recent-campaign rates always cover each campaign's complete recorded history. The clickable information control explains these counting rules.

For 7-day, 30-day and 365-day selections, smaller absolute count changes compare with the preceding equal-length period. That previous cohort includes its lower boundary and excludes the current period's lower boundary; its observations also stop before that boundary. Both periods are read in one transaction. Late interactions do not retrospectively inflate the previous period, repeated sends do not move recipients between periods, and a zero previous count remains a valid baseline. All time has no comparison. Arrows indicate numerical direction. Increasing opens, clicks or credentials-entered counts are red; decreases are green. Increasing reports are green; decreases are red. Unchanged counts stay neutral. Emails sent shows its total only; the API does not expose a previous-period send count. These are absolute count comparisons, so the accompanying percentages provide context when send volume changes.

## Navigation and loading

The Dashboard link sits above the sidebar groups. The analysis heading opens `/analysis`, the recent-campaign heading opens `/campaigns`, each campaign name opens its campaign. The single row action opens Campaign for unfinished campaigns (including scheduled ones) and Analysis for completed campaigns. Resources have a dedicated card with green, underlined resource-name links. Their counts are plain text; no separate View or arrow buttons are shown. Attention actions link to the relevant management pages. Recent campaigns and Needs attention stretch to the same height when displayed alongside each other. Creating a campaign requires the existing modification permission.

Snapshot and metric requests load independently. Changing the period reloads only the metrics. A failed refresh preserves the last confirmed response and its timestamp, displays the error and offers a retry. Changing periods clears old-period values immediately; late responses cannot replace the current selection. Requests use the shared bounded transport and session handling, and are cancelled when the page is removed.

## Read API

| Route | Response | Scope |
| --- | --- | --- |
| `GET /api/overview` | `Snapshot` | Campaign counts, five recent campaigns, inventory, attention and `as_of` |
| `GET /api/overview/posture?window=all` | `PostureResponse` | Acceptance cohort and aggregate observations, `window`, `since`, `as_of` and optional `previous` counts/bounds |

Both routes use the existing authenticated browser/API boundary, require `view_objects`, return JSON with `Cache-Control: no-store`, and never mutate resources or produce interaction events. Missing or empty `window` means `all`; `all`, `7d`, `30d` and `365d` are the accepted values. The existing `api_key` query authentication parameter is also accepted. Invalid, repeated or unknown posture parameters return HTTP 400 with `overview_invalid_window`. Authentication is checked before window validation. A database/read failure is an error, not a successful zero count.

Scheduled campaign counts derive from future launch times for unfinished campaigns, including persisted `Queued` campaigns. Attention includes enabled domains without current verification and failed, blocked or uncertain deliveries in unfinished campaigns. Disabled domains remain in inventory but do not request verification attention. Domain verification and enablement remain separate attributes.

Typed wire contracts live in `ruphish-contracts::overview`. The application repository defines the authorized read operations; SQLite supplies consistent transactional reads; the HTTP layer maps into the wire types. The existing seven crate boundaries remain unchanged.
