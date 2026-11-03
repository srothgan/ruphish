---
title: Understand the metrics
description: Distinguish observed events, participant counts, progression, and delivery outcomes.
sidebar:
  order: 32
---

Ruphish records delivery and interaction events, then summarizes them for different views. A count only becomes useful when you know what it counts.

## What each observation means

| Observation | What Ruphish recorded | What it does not prove |
| --- | --- | --- |
| Email sent | The SMTP server accepted the message, or an operator confirmed acceptance. | Arrival in the inbox or a person reading it. |
| Opened | A request for the email's tracking image. | A human deliberately reading the message. |
| Clicked | A visit to the campaign landing page. | A deliberate human click rather than scanning or automation. |
| Submitted | A supported form-submission event. | Valid credentials or successful sign-in. |
| Reported | A report notification matched to the recipient. | The identity or intention of the person who forwarded it. |

Password presence is a separate yes/no observation on a submission. A submission alone does not mean a password was entered.

## People and events are different counts

The campaign **Results** cards count each recipient once for each recorded action within the selected filters. The chart uses the first recorded occurrence of each action per recipient in the selected period.

Individual recipient histories and event exports retain repeated events. If Alex clicks three times and Sam clicks once, that is **two recipients with clicks** and **four click events**.

The same email address included in different campaigns is a separate campaign recipient each time. Adding counts across campaigns does not give a count of distinct people across the installation.

## Observed activity and progression

The campaign **Results** view uses recorded events. A recorded submission does not create a missing click or open in that view.

Other progression summaries, including the dashboard's campaign outcomes, can count a submission as having reached the clicked stage, and a click or submission as having reached the opened stage. These inferences describe progression; they do not add events or invent timestamps.

This is why a progression count can differ from a filtered Results count. SMTP acceptance and reports are never inferred from later interactions.

## Rates need a denominator

The dashboard's recent-campaign rates divide by that campaign's recorded sent-recipient count. A zero denominator displays a dash.

For example, 10 recipients with clicks out of 100 recorded sent recipients gives a 10% click rate. Repeated clicks do not increase the numerator.

When comparing campaigns, check their audiences, tracking settings, and selected periods. A filtered observation count and a whole-campaign percentage answer different questions.

## When opens are not tracked

If **Track email opens** was off at launch, open metrics show **Not tracked** or a dash. This means the measurement was disabled, not that nobody opened the message.

Plain-text-only email has no image-based tracking even when the campaign setting is enabled. Image blocking, caching, and mail proxies also affect open observations. Stored or manually added open events do not establish that automatic tracking covered every message.

## Corrections and sources

Events can come from public tracking, reporting mailboxes, application actions, imports, or manual additions. Use the source filter when the distinction matters.

Editing an event changes its current values. Deleting it removes that event from the results. Charts and exports use corrected times and current events; they are not an audit archive of every previous value.

An imported reported flag without a dated report event may appear separately. It cannot supply an invented report time.

## Present findings accurately

Report what was observed and explain the scope. For example: "12 recipients had recorded clicks during the selected period" is more precise than "12 people fell for the message."

Read [Participant data](participant-data.md) for collection limits and [View campaign results](view-campaign-results.md) for filter behavior.
