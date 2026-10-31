---
title: Campaigns API
description: Prepare, launch, inspect, complete, and resolve campaign delivery through the API.
sidebar:
  order: 10
---

A campaign is an execution with a fixed audience and selected resources. Use [campaign drafts](campaign-drafts.md) for incomplete work that must not send messages yet.

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| GET, POST | `/api/campaigns/` | List executions or launch one |
| POST | `/api/campaigns/prepare` | Validate selections, audience, policy, schedule, and preview without launching |
| POST | `/api/campaigns/test` | Send a test from the selected campaign configuration |
| GET | `/api/campaigns/{id}` | Legacy execution detail |
| DELETE | `/api/campaigns/{id}` | Delete execution data |
| GET | `/api/campaigns/{id}/context` | Typed campaign configuration and linked-resource context |
| GET | `/api/campaigns/{id}/results` | Filtered native results, chart buckets and bounded recipient/event pages |
| GET | `/api/campaigns/summary` | Legacy collection summaries |
| GET | `/api/campaigns/{id}/summary` | Legacy execution summary |
| GET | `/api/campaigns/overview` | Bounded execution headers; `after`, `limit`, and optional `completed` |
| GET | `/api/campaigns/directory` | Paged mixed draft/execution directory |
| GET | `/api/campaigns/{id}/draft` | Read inputs for duplication; does not save a draft |
| POST | `/api/campaigns/{id}/complete` | Complete a campaign |
| GET | `/api/campaigns/{id}/delivery` | Delivery state and attempts |
| GET | `/api/campaigns/{id}/recipients/{result}/history` | Recipient attempts and current individual events, including read-only client categories |
| POST | `/api/campaigns/{id}/recipients/{result}/events` | [Add, edit or delete an event](analysis-and-exports.md#event-corrections) |
| POST | `/api/campaigns/{id}/delivery/{result}` | Resolve or retry a delivery outcome |

`/complete` accepts POST only; GET and HEAD return `405` without changing campaign state. Requests in this route family have a 1 MiB body limit.

## Read a campaign

Set the connection variables from [your first API request](index.md#make-your-first-request), then replace the dummy ID with an existing campaign ID:

```sh
export CAMPAIGN_ID='123'

curl --header "Authorization: Bearer $RUPHISH_API_KEY" \
  "$RUPHISH_URL/api/campaigns/$CAMPAIGN_ID"
```

This request reads campaign details without launching or changing the campaign. Include the private-CA option from the setup example if your installation requires it.

## Prepare and launch

Campaign requests select owned resources by ID. Use the same body for preparation and direct launch:

```json
{
  "name": "Local exercise",
  "selection": {
    "groups": [{"id": 1}],
    "template": {"id": 2},
    "page": {"id": 3},
    "smtp": {"id": 4}
  },
  "url": "https://campaign.test:8443"
}
```

This body is illustrative: replace the example IDs with resources owned by the caller, and the URL must be an eligible campaign origin under the current [verification policy](settings.md). Posting it to `/api/campaigns/` schedules real sending. Use the [local fixture](../getting-started/first-campaign.md) for a contained trial.

`selection` is required: `groups` is an array, and `template`, `page`, and `smtp` are objects with `id` and an optional revision string. Preparation returns resolved selections, `total`, `duplicate_count`, `conflict_count`, conflicting recipient details, resolved policy and schedule, `mailer_enabled`, and a rendered `preview`. Carry each selected `id` and `revision` into launch so changed content is detected. Review selections also include display names; omit those names from command inputs. Resource renaming does not change the selected identity. Missing or foreign-owned resources are rejected; there is no fallback lookup by name.

Request fields are case-sensitive. Unknown fields, duplicate fields, invalid types and trailing JSON are rejected with `400`. Top-level legacy `groups`, `template`, `page` and `smtp` inputs are no longer accepted, including alongside `selection`. The same typed campaign input is used inside test-send and draft-launch requests.

The optional `policy` contains `track_opens` (Boolean, default `true`), `redirect_url`, and an optional owned `learning_page_id`. `track_opens: false` disables managed email pixels and public open observations while leaving clicks, submissions, and reports independent. The setting is fixed at launch, returned in confirmed policy reads, and preserved by campaign copying and test sends. `null` is not a valid tracking choice. A learning page and a nonempty custom redirect are mutually exclusive. Every supported submission records an event and password presence; there are no capture settings. Campaign commands reject `capture_credentials` and `capture_passwords` fields. See [landing pages](landing-pages.md).

Saved draft policies can retain `track_opens` while `redirect_url` is explicitly `null`, representing unresolved page defaults. Such drafts remain saveable but cannot launch until the destination choice is resolved and saved. An empty redirect string is a confirmed choice of no redirect.

`launch_date` and `send_by_date` accept UTC or offset timestamps. Alternatively, `launch_local` and `send_by_local` each contain `local` and `time_zone`. For each boundary, supply at most one non-null representation: combining `launch_date` with `launch_local`, or `send_by_date` with `send_by_local`, returns `400` with validation issues for both fields, even if they describe the same instant. This applies to preparation, direct creation, test sends and draft launch. Different boundaries may use different representations. If both launch fields are omitted or null, launch is immediate; if both send-by fields are omitted or null, there is no distribution window.

Local inputs are resolved through the named IANA zone. Repeated civil times use the first occurrence; nonexistent times shift forward. Review the resolved schedule before launching. Read [scheduling](../user-guide/campaigns/scheduling.md) before relying on distribution timing.

For direct creation, supply an `Idempotency-Key` and retain it with the original request when recovering from a lost response. Do not reuse one operation's key for a different campaign request. For saved drafts, the draft identity owns launch idempotency instead.

Direct creation returns `201`, while draft launch returns `200`. An idempotency key must be nonempty and at most 128 bytes. The response to direct creation omits the timeline and includes the resolved groups. A repeated key does not authorize a new execution after deletion.

`POST /api/campaigns/test` takes an object containing `campaign` (the campaign request) and `email` (the controlled test recipient). It uses sample personalization values and returns a `200` success envelope after SMTP acceptance. It sends a real test message without launching the campaign; a lost response is not a reason to resend automatically.

## Learning-page destinations

Set `policy.learning_page_id` to an owned [learning page](learning-pages.md) to use its hosted explanation after submission. A nonempty custom `redirect_url` cannot be combined with that selection. The campaign destination is fixed at launch; the reusable page content remains editable. Omitting the learning-page field preserves legacy redirect behavior.

## Read results

Campaign detail includes `track_opens`, identity, lifecycle timestamps, URL, selected resources, and available `results` and `timeline`. Absent lifecycle dates return `null`, including `send_by_date` and `completed_date`. Empty results or timeline fields may be omitted. Result `id` is the opaque recipient tracking ID. Legacy network-location fields are not a source of participant identity; do not build new tracking around them.

Legacy summaries also return `track_opens`; use it to distinguish disabled open tracking from measured zero activity. Legacy summary `stats` includes `total`, `sent`, `opened`, `clicked`, `submitted_data`, `email_reported`, and `error`. These use progression semantics. Use [results](analysis-and-exports.md) for observed event counts, filters, and exportable rows; do not combine the two models as if they were identical metrics.

The directory accepts `filter=all`, `drafts`, `active`, `completed`, or `executions` and an opaque `after` cursor. It returns `items` and `next_after`, with at most 50 rows per page. Keep cursor values opaque.

For sorted directory pages, add `sort=name`, `status`, `total`, or `launch` and optional `order=asc` (default) or `desc`. Pass the returned `next_after` unchanged as the next request's `after`. The cursor is bound to the owner, filter, column and direction; clear it when any selection changes. Sorting covers the full matching directory before its 50-row page is selected. Without `sort`, the existing draft-first ID traversal remains available; `order` requires `sort`.

Sorted continuations retain the boundary value and row identity, so deleting the boundary row does not invalidate traversal. Equal values use stable identities; absent audience/schedule values precede present values ascending. These are live queries, not a snapshot spanning requests: changes to names, status, schedules or audience sizes can move rows across the current boundary. Treat cursor strings as opaque and URL-encode them; malformed or mismatched sorted cursors return `400`.

`/api/campaigns/overview` is a different execution-only read. It accepts numeric `after`, optional `limit` (default 50), and optional boolean `completed`. Its response contains header `items`, numeric/null `next_after`, and `mailer_enabled`. Headers include identity, status, schedule, URL, resource references, audience total, and available policy/domain-block metadata. Unknown historical policy is `null`, including in the editable copy returned by `/api/campaigns/{id}/draft`; `{}` is not a policy value.

## Delivery and uncertain outcomes

Delivery reads use numeric result-row IDs. The page includes a campaign header, sparse state `counts`, `items`, optional `next_after`, policy, `mailer_enabled`, and recent activity. Rows expose `id`, `rid`, `email`, `state`, `due` (UTC Unix seconds), `attempt_count`, `retries`, `generation`, `code`, and recent `attempts`. Missing state-count keys mean no rows in that state. Queue states are `ready`, `preparing`, `submitting`, `retry`, `accepted`, `failed`, `unknown`, `blocked`, and `cancelled`. They are separate from campaign lifecycle (`Created`, `Queued`, `In progress`, `Emails Sent`, `Completed`) and draft state (`draft`, `launched`). The directory's `kind` selects the corresponding lifecycle; a draft cannot have an execution status.

To sort delivery pages, send `sort=email`, `state`, `due`, or `attempt_count` with optional `order=asc` (default) or `desc`, and `limit` from 1 to 200 (default 50). The response adds nullable `next_cursor`; supply it as `cursor` on the next request with the same campaign, owner and ordering. Do not combine a sorted cursor with a nonzero numeric `after`. Sort/filter changes start again without a cursor. Equal values use ascending result ID; due times and attempt counts sort numerically. The original numeric `after`/`next_after` path remains available when `sort` is omitted.

History accepts `before_attempt` and `before_event` cursors and returns attempts, events, and separate next cursors. Attempt fields include generation, start/finish times, state, SMTP stage, optional SMTP code, and message ID. Attempt states are `preparing`, `submitting`, `retry`, `accepted`, `failed`, `unknown`, `blocked`, `cancelled`, and `abandoned`. An abandoned attempt means its preparation lease expired before submission; `ready` belongs to the queue, not an attempt.

A resolution POST contains the current `generation` and an `action`: `confirm_accepted` or `resend_unknown` for an unknown outcome, or `retry` for an eligible failed/blocked result. Reload before resolving a stale generation. An unknown outcome can mean SMTP accepted the message before the connection failed; resending may produce a duplicate.

Completion stops remaining campaign work but cannot recall accepted mail. Deletion removes history. See [completion and deletion](../user-guide/campaigns/completion-and-deletion.md) before invoking either operation.

The compact context read returns the launched schedule, audience size, policy and resource references. Template subject and sender/profile information describe current shared resources. It excludes resource bodies and service credentials. Recipient history pages contain at most 100 attempts and 100 events each; use `next_attempt` and `next_event` as the exclusive `before_attempt` and `before_event` cursors. Numeric recipient identities belong to the campaign, and cross-campaign or cross-owner reads fail with `404`.
