---
title: Schedule delivery
description: Choose launch times and delivery windows, including time-zone and restart behavior.
sidebar:
  order: 4
---

Leave the launch time empty for immediate delivery, or choose a future local date/time. Review resolves local input through the selected IANA time zone and shows the effective schedule. Changing your display preferences later does not move an existing campaign.

## Time zones and daylight saving

Stored/API instants are UTC. The interface presents them in your account's selected zone or browser-detected zone. A formatting locale controls appearance independently of the zone.

For repeated local times during a daylight-saving change, local scheduling uses the first occurrence. A nonexistent local time in a forward gap is shifted forward. Check the resolved time and UTC offset in review, especially near a transition. API clients can send an explicit offset timestamp when they need an unambiguous instant.

## Spread the messages out

An optional send-by time distributes initial due times across the audience. The distribution uses the unique resolved audience and minute-level spacing. Small audiences or short windows can share a due time; the final recipient need not be scheduled at the exact send-by instant.

The send-by time is not a guaranteed delivery deadline. SMTP retries, domain verification, outages, and worker capacity can delay acceptance beyond it. Without a send-by time, all initial deliveries are due at launch and workers process them within their bounded capacity.

## Restarts and paused delivery

The queue persists. Restarting does not forget future schedules. Work that is due can resume when workers and required domain evidence are available. Only processes containing the administration listener run the campaign worker, and `--disable-mailer` stops that worker in the selected process.

A draft remains a draft when its proposed time passes. Replace the past time or clear it before launch. After launch, use [delivery status](delivery.md) to distinguish waiting work from failed or uncertain submissions.
