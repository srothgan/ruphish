---
title: Compatibility with Gophish
description: Understand the goal of Gophish compatibility, supported contracts, and intentional differences.
sidebar:
  order: 2
---

Ruphish aims to be compatible with Gophish and serve as a drop-in replacement for existing installations. Compatibility work preserves HTTP resource contracts, Go-template behavior, and supported database histories while adding new capabilities. Full compatibility cannot be guaranteed for every integration, configuration, or deployment. Intentional changes, including participant-data restrictions and domain verification, can require adjustments before migration.

The comparison reference is Gophish 0.12.1 at commit `95618469799295e2c0fec980805a2dfbb818816b`. Use the table below and the [documented differences](differences-from-gophish.md) to assess your installation.

| Area | Current boundary | What to verify before cutover |
| --- | --- | --- |
| HTTP resources | Groups, templates, pages, SMTP profiles, campaigns, users, and integration compatibility routes remain; Ruphish adds new endpoints | Each method your client calls, auth form, status handling, empty fields, and response parsing |
| API layout | `/api/` exposes public operations; `/auth/` supplies browser authentication/session data; `/internal/` serves browser-specific operations | External clients use documented `/api/` routes rather than browser internals |
| Templates | Go-template syntax is a compatibility requirement, including expressions and control structures | Render your actual templates and attachments; inspect missing fields, escaping, and functions |
| SQLite history | Recognized legacy histories upgrade with a recoverable snapshot | Rehearse against a copy and check identities, relationships, timestamps, and usable account hashes |
| MySQL history | Recognized legacy histories upgrade through staged publication and retained originals | Supported server version, schema preflight, required privileges, timestamp interpretation, and recovery |
| PostgreSQL | A Ruphish provider for fresh installations and Ruphish migrations | It is not a Gophish database importer or a cross-engine transfer tool |
| Campaign behavior | Durable delivery, retry state, restart recovery, and explicit uncertain outcomes | Queued, active, completed, and ambiguous legacy delivery cases |
| Participant data | Interaction events and optional presence booleans replace legacy value capture | Rework pages and downstream reports that expected entered values or IP-derived location |
| Integrations | Signed webhooks and IMAP reporting have explicit persistence and retry behavior | Receiver deduplication, mailbox matching, TLS trust, and credential handling |
| Existing API clients | Compatibility with Gophish clients is a goal; the HTTP contract defines supported operations | Exercise the client versions and methods you use, and use the added endpoints for Ruphish-specific features |

Compatibility fixtures and tests cover specific contracts and workflows. Contributors can use the [verification guidance](../contributing/testing/index.md) to run the relevant checks.

For a migration decision, record the exact source/target versions, database engine and history, exercised client versions, and results of your rehearsal. Treat untested combinations as unverified. See [intentional differences](differences-from-gophish.md) before deciding whether a client or content change is required.
