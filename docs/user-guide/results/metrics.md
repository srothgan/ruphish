---
title: Metric definitions
description: Understand observed events, progression counts, denominators, and reporting limitations.
sidebar:
  order: 2
---

Ruphish separates recorded observations from summary progression. That distinction explains why a summary can show an opened recipient even when their timeline has no separate open event.

| Observation | What was recorded |
| --- | --- |
| Accepted/sent | SMTP acceptance |
| Opened | A tracking-image request |
| Clicked | A campaign landing-page visit |
| Submitted | A supported presence-only form event |
| Reported | A report notification |

## Counts and progression

Raw event totals include repeated observations. Unique counts include each attributable recipient once for the selected event type. Progression counts infer that a clicked or submitted recipient has reached the opened stage, and a submitted recipient has reached the clicked stage. Acceptance and reports are never inferred.

Campaign Results summary cards, the dashboard, and the campaign execution overview count each recipient at most once per category within a campaign. Recipient detail counts and event history count all recorded observations, including repeats. A retry of the same submission delivery is deduplicated. Campaign Results retains separately labelled event totals and activity charts.

Those inferences do not add timeline events or invent timestamps. Charts, event tables, and event exports retain recorded observations. Password presence requires an explicit positive boolean; submission alone does not mean credentials were entered.

## Dashboard scope

The dashboard shows compact rates for recent campaigns. Each rate uses that campaign's recorded sent-recipient count; a zero denominator displays a dash. The same address in different campaigns counts separately.

## Campaign Results scope

Results applies recipient, event, source, and time filters before aggregation and pagination. Its lower time bound is inclusive; its upper bound is exclusive. Recipient detail shows that recipient's full observation history and delivery attempts; the campaign's selected filters remain available when returning.

## Historical and automated activity

An imported reported flag without a corresponding report event is shown separately; no report timestamp is invented. Ambiguous legacy records cannot be assigned to a recipient by guesswork, so unique-recipient statistics exclude unresolved attribution while event exports can retain those observations.

Scanners, proxies, remote-image settings, and caching can create or suppress requests. Treat rates as observations under these constraints, not as a measurement of a person's intentions or password validity.
