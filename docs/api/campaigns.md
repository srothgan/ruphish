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
| GET | `/api/campaigns/{id}/results` | Legacy results and timeline |
| GET | `/api/campaigns/summary` | Legacy collection summaries |
| GET | `/api/campaigns/{id}/summary` | Legacy execution summary |
| GET | `/api/campaigns/overview` | Bounded execution headers; `after`, `limit`, and optional `completed` |
| GET | `/api/campaigns/directory` | Paged mixed draft/execution directory |
| GET | `/api/campaigns/{id}/draft` | Read inputs for duplication; does not save a draft |
| POST | `/api/campaigns/{id}/complete` | Complete a campaign |
| GET | `/api/campaigns/{id}/delivery` | Delivery state and attempts |
| GET | `/api/campaigns/{id}/delivery/{result}/history` | Recipient attempt and event history |
| POST | `/api/campaigns/{id}/delivery/{result}` | Resolve or retry a delivery outcome |

The legacy GET form of `/complete` also mutates the campaign. Use POST in new integrations. Requests in this route family have a 1 MiB body limit.

## Read a campaign

Set the connection variables from [your first API request](index.md#make-your-first-request), then replace the dummy ID with an existing campaign ID:

```sh
export CAMPAIGN_ID='123'

curl --header "Authorization: Bearer $RUPHISH_API_KEY" \
  "$RUPHISH_URL/api/campaigns/$CAMPAIGN_ID"
```

This request reads campaign details without launching or changing the campaign. Include the private-CA option from the setup example if your installation requires it.

## Prepare and launch

The familiar request selects resources by name:

```json
{
  "name": "Local exercise",
  "groups": [{"name": "Local tutorial"}],
  "template": {"name": "Local invitation"},
  "page": {"name": "Local exercise"},
  "smtp": {"name": "Local inbox"},
  "url": "https://campaign.test:8443"
}
```

This body is illustrative: the named resources must exist under the caller, and the URL must be an eligible verified campaign origin. Posting it to `/api/campaigns/` schedules real sending. Use the [local fixture](../getting-started/first-campaign.md) for a contained trial.

New clients should use `selection` with explicit resource references: `groups` is an array, and `template`, `page`, and `smtp` are objects with `id` and an optional revision string. Preparation returns resolved selections, `total`, `duplicate_count`, `conflict_count`, conflicting recipient details, resolved policy and schedule, `mailer_enabled`, and a rendered `preview`. Carry its selected revisions into launch so changed content is detected.

The optional `policy` contains `capture_credentials`, `capture_passwords`, and `redirect_url`. The flags are ignored compatibility inputs; every supported submission records an event and password presence. Only `redirect_url` changes effective policy. See [landing pages](landing-pages.md).

`launch_date` and `send_by_date` accept timestamp values. A missing or zero launch date means immediate launch. Alternatively, `launch_local` and `send_by_local` each contain `local` and `time_zone`. Local inputs are resolved through that IANA zone and checked for ambiguous or nonexistent civil times. Read [scheduling](../user-guide/campaigns/scheduling.md) before relying on distribution timing.

For direct creation, supply an `Idempotency-Key` and retain it with the original request when recovering from a lost response. Do not reuse one operation's key for a different campaign request. For saved drafts, the draft identity owns launch idempotency instead.

Direct creation returns `201`, while draft launch returns `200`. An idempotency key must be nonempty and at most 128 bytes. The response to direct creation omits the timeline and includes the resolved groups. A repeated key does not authorize a new execution after deletion.

`POST /api/campaigns/test` takes an object containing `campaign` (the campaign request) and `email` (the controlled test recipient). It uses sample personalization values and returns a `200` success envelope after SMTP acceptance. It sends a real test message without launching the campaign; a lost response is not a reason to resend automatically.

## Read results

Legacy campaign detail includes identity, lifecycle timestamps, URL, selected resources, and available `results` and `timeline`. Empty results or timeline fields may be omitted. Result `id` is the opaque recipient tracking ID. Legacy network-location fields are not a source of participant identity; do not build new tracking around them.

Legacy summary `stats` includes `total`, `sent`, `opened`, `clicked`, `submitted_data`, `email_reported`, and `error`. These use progression semantics. Use [analysis](analysis-and-exports.md) for observed event counts, filters, and exportable rows; do not combine the two models as if they were identical metrics.

The directory accepts `filter=all`, `drafts`, `active`, `completed`, or `executions` and an opaque `after` cursor. It returns `items` and `next_after`, with at most 50 rows per page. Keep cursor values opaque.

`/api/campaigns/overview` is a different execution-only read. It accepts numeric `after`, optional `limit` (default 50), and optional boolean `completed`. Its response contains header `items`, numeric/null `next_after`, and `mailer_enabled`. Headers include identity, status, schedule, URL, resource references, audience total, and available policy/domain-block metadata.

## Delivery and uncertain outcomes

Delivery reads use numeric result-row IDs. The page includes a campaign header, sparse state `counts`, `items`, optional `next_after`, policy, `mailer_enabled`, and recent activity. Rows expose `id`, `rid`, `email`, `state`, `due` (UTC Unix seconds), `attempt_count`, `retries`, `generation`, `code`, and recent `attempts`. Missing state-count keys mean no rows in that state.

History accepts `before_attempt` and `before_event` cursors and returns attempts, events, and separate next cursors. Attempt fields include generation, start/finish times, state, SMTP stage, optional SMTP code, and message ID.

A resolution POST contains the current `generation` and an `action`: `confirm_accepted` or `resend_unknown` for an unknown outcome, or `retry` for an eligible failed/blocked result. Reload before resolving a stale generation. An unknown outcome can mean SMTP accepted the message before the connection failed; resending may produce a duplicate.

Completion stops remaining campaign work but cannot recall accepted mail. Deletion removes history. See [completion and deletion](../user-guide/campaigns/completion-and-deletion.md) before invoking either operation.
