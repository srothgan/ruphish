---
title: Overview API
description: Read owner-scoped dashboard activity and delivered-recipient posture metrics.
sidebar:
  order: 14
---

Ruphish provides dashboard-specific endpoints so clients do not need to download every campaign to construct a summary.

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/overview` | Campaign inventory, recent executions, and attention items |
| GET | `/api/overview/posture` | Delivered-recipient cohort metrics and previous-window comparison |

Only `/api/overview/posture` accepts `window=all`, `7d`, `30d`, or `365d`. Its API default is `all`; the UI initially selects 30 days. Unknown or repeated window values are rejected there. `/api/overview` has no period filter and does not validate a supplied window parameter. Responses are owner-scoped, authenticated, and not cacheable as public content.

## Overview snapshot

`as_of` is UTC Unix seconds. `recent` contains at most five campaigns with identity, status, launch, recipients, and outcome counts. `inventory` counts templates, pages, groups, profiles, domains, verified domains, and reporting mailboxes.

Recent campaign rows use `launch: null` when the scheduled time is unknown or absent; known launch times are normalized UTC strings.

`attention` contains enabled unverified-domain and delivery-problem counts, plus the next launch when one exists. These are navigation cues; open the relevant campaign or domain for the actionable state.

## Posture snapshot

The response includes `window`, optional inclusive `since`, `as_of`, `delivered`, `opened`, `clicked`, `reported`, and `credentials_entered`. A bounded window can include `previous` with its own `since`, exclusive `until`, and outcome counts.

The cohort uses each campaign recipient's first SMTP acceptance time. Repeated events do not move that recipient into a newer cohort. The same address in two campaigns is two campaign recipients. Observations are counted through the snapshot time, with progression rules for earlier engagement stages.

Rates use delivered recipients as their denominator. A zero denominator is unavailable, not a measured 0% outcome. `credentials_entered` is a legacy metric name for the tracked presence outcome; it does not contain credentials. Read [dashboard metrics](../user-guide/results/dashboard.md) before comparing these values with raw analysis event counts.
