# Save and resume a campaign

Open **Campaigns → Create campaign**. Moving between editing steps saves your inputs as a draft. **Save as draft** saves the current step and opens the draft view; an existing draft uses **Save changes**. The save indicator shows unsaved changes, an active save, or the last confirmed save time.

A draft can be incomplete. It has no delivery jobs, recipient tracking identifiers, campaign events, or analysis results. An entered launch time is only a proposed schedule until you explicitly launch the campaign. Passing that time never starts a draft automatically.

## Resume, review, and launch

Campaigns shows drafts and launched campaigns in one table, with drafts first and an amber **Draft** badge. The heading and compact lifecycle filter sit above the table. One pagination control covers the combined results. Draft rows offer View, Edit and Delete; launched rows offer View, Analysis and Delete. Deleting a row refreshes the list immediately. Use the status filter to show drafts, active campaigns, or completed campaigns. Open a draft to view its saved settings, edit it, or delete it. Editing resumes at the saved step. The active campaign and analysis pages remain separate from draft editing.

Entering the review step automatically validates current resource selections and prepares the audience and preview. Missing resources must be replaced. A saved time in the past must be corrected or cleared for immediate delivery. Review is refreshed when its inputs change and is never persisted as lasting authorization to launch.

**Launch campaign**, or **Schedule campaign** for a future time, creates the execution state in one transaction. The draft then links to that campaign and can no longer be edited or deleted as a draft. If the response is lost, retrying the same launch returns the existing campaign. Refreshing the saved draft also resolves to its campaign. This prevents duplicate campaign creation; SMTP delivery still has its uncertain-outcome cases.

## Recover from a failed save

A failed save keeps the form values. Retry after resolving the connection or validation error. Changes made after the last confirmed save remain unsaved and are subject to the shared navigation warning.

Directory and saved-detail refreshes keep the last confirmed data when a read fails, show a localized stale-data message, and offer retry. Changing the directory filter or page does not display another selection's rows. A confirmed deletion removes the draft from cached directory pages before refreshing them. Background resource refreshes update the available choices without replacing unsaved draft fields.

If another tab changed the draft, the server rejects a stale edit with `RESOURCE_CHANGED`. Keep the unsaved values before reloading and reconciling with the newer saved version. A deleted resource does not erase the draft; select a replacement before launch. Deleting a draft removes its configuration without affecting any existing campaign.

## Draft API

These additive endpoints require the same authenticated ownership and resource permissions as campaigns. Browser mutations require the normal CSRF and origin checks. The legacy `/api/campaigns/` API retains its existing immediate-create behavior and does not return drafts.

| Method and path | Behavior |
| --- | --- |
| `GET /api/campaigns/directory?filter=all&after=<cursor>` | Combined management metadata with one 50-row page; filters are `all`, `drafts`, `active`, `completed`, and execution-only `executions`. Use the returned opaque `next_after` unchanged. |
| `GET /api/campaign-drafts?after=<id>` | Up to 50 editable drafts with an optional `next_after` cursor. |
| `GET /api/campaign-drafts/{id}` | Saved configuration, revision, editing step, UTC timestamps, and nullable `campaign_id`. |
| `PUT /api/campaign-drafts/{id}` | Save `{ "revision": null, "configuration": {}, "step": 0 }` to create an incomplete draft. Use the returned revision for subsequent saves. |
| `DELETE /api/campaign-drafts/{id}?revision=<revision>` | Delete an unchanged, unlaunched draft. |
| `POST /api/campaign-drafts/prepare` | Review campaign input without saving or launching; reject a past launch time. |
| `POST /api/campaign-drafts/{id}/launch` | Submit `{ "revision": 1, "campaign": { ... } }`, including the current reviewed selection revisions. Launch input must match the saved configuration. |

Choose a client-generated draft ID of 16–64 ASCII letters, digits, or hyphens and retain it across retries. Repeating the same save against the preceding revision acknowledges the persisted result; changed input with a stale revision is rejected. Launch identity is the draft itself, independently of the legacy idempotency-key namespace.

Configuration accepts `name`, `url`, `selection`, `policy`, `launch_local`, and `send_by_local`. Selection holds ordered group references and template, page, and SMTP references containing only `id`; zero means unselected. Policy holds `capture_credentials`, `capture_passwords`, and `redirect_url`, or null while unresolved. Local schedules contain `local` and `time_zone`. Unknown fields, service credentials, invalid field types, and excessive inputs are rejected with `DRAFT_INVALID`. Drafts permit incomplete local dates; review requires valid dates. Past launch times produce `DRAFT_SCHEDULE_PAST`. Names are limited to 255 characters, URLs and redirect expressions to 8,192 characters, group selections to 1,000, and local-time fields to 100 characters each. Editing steps are 0–3.

The database upgrade adds separate versioned draft storage on SQLite, MySQL, and PostgreSQL. Existing campaigns and their histories are not converted into drafts. Account deletion removes that account's drafts; deleting a launched campaign does not make its old draft launchable again.
