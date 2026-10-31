---
title: Compatibility with Gophish
description: Understand supported Gophish data and template migration and adapt integrations to the Ruphish API.
sidebar:
  order: 2
---

Ruphish supports familiar campaign workflows, authored Go-template behavior, and recoverable migration of recognized Gophish databases. Its HTTP API uses Ruphish contracts; there is no separate Gophish-compatible API version. Adapt existing clients to the documented request and response types.

The comparison reference is Gophish 0.12.1 at commit `95618469799295e2c0fec980805a2dfbb818816b`. Use the table below and the [documented differences](differences-from-gophish.md) to assess your installation.

| Area | Current boundary | What to verify before cutover |
| --- | --- | --- |
| HTTP resources | Familiar resource families remain, with native contracts; campaign commands require ID-based selections | Use the current endpoint documentation, including request types, authorization, statuses and response parsing |
| API layout | `/api/` exposes public operations; `/auth/` supplies browser authentication/session data; `/internal/` serves browser-specific operations | External clients use documented `/api/` routes rather than browser internals |
| Templates | Go-template syntax is a compatibility requirement, including expressions and control structures | Render your actual templates and attachments; inspect missing fields, escaping, and functions |
| SQLite history | Recognized Gophish histories convert directly to release v1 with a recoverable snapshot | Rehearse against a copy and check identities, relationships, timestamps, and usable account hashes |
| MySQL history | Recognized Gophish histories convert directly to release v1 through staged publication and retained originals | Supported server version, schema preflight, required privileges, timestamp interpretation, and recovery |
| PostgreSQL | A Ruphish provider for fresh installations and Ruphish migrations | It is not a Gophish database importer or a cross-engine transfer tool |
| Campaign behavior | Durable delivery, retry state, restart recovery, and explicit uncertain outcomes | Queued, active, completed, and ambiguous legacy delivery cases |
| Participant data | Interaction events, password-presence booleans, and coarse per-event client categories replace legacy value/location capture | Rework pages and downstream reports that expected entered values or IP-derived location |
| Integrations | Creator-owned configuration under administrator feature switches; webhook delivery and mailbox matching span campaign creators | Migrated webhook ownership, receiver deduplication, initial mailbox boundaries, closed-campaign rejection, ambiguous forwards, TLS trust, and credential handling |
| Existing API clients | Clients use the documented Ruphish API | Replace name-only campaign requests with [ID selections](../api/campaigns.md#prepare-and-launch) and test each integration |

Compatibility fixtures and tests cover specific contracts and workflows. Contributors can use the [verification guidance](../contributing/testing/index.md) to run the relevant checks.

For a migration decision, record the exact source/target versions, database engine and history, exercised client versions, and results of your rehearsal. Treat untested combinations as unverified. See [intentional differences](differences-from-gophish.md) before deciding whether a client or content change is required.

Conversion supports the 25 recorded Gophish SQLite schema versions and 28 Gophish MySQL schema versions. Startup validates both schema and migration history; unrecognized or inconsistent databases are rejected. No automatic cross-engine conversion is provided.
