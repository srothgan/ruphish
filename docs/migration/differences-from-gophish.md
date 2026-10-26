---
title: What changes from Gophish
description: Understand Ruphish's expanded interface, new APIs, localization, and changed operational behavior.
sidebar:
  order: 3
---

Ruphish keeps the campaign concepts while replacing both the server and administration interface. These changes matter when you bring across existing habits, content, or automation.

The comparison baseline is Gophish 0.12.1, commit `95618469799295e2c0fec980805a2dfbb818816b`. Ruphish aims to be a compatible, drop-in replacement; the intentional differences below identify where your configuration, content, or integrations may need changes.

| Area | Gophish baseline | Ruphish behavior | Required user action |
| --- | --- | --- | --- |
| UI | Legacy web administration and campaign views | Leptos/WASM interface with saved drafts, delivery history/reconciliation, domains, richer analysis, and multiple mailboxes | Follow current guides and replace old screen-specific instructions |
| Localization | No equivalent account-level language/format/zone preference workflow | English/German interface with independently persisted format and time-zone choices | Choose account preferences; keep API timestamps and names unchanged |
| Browser API layout | Browser and administrative routes without the same three-way separation | Public `/api/`, session `/auth/`, browser-specific `/internal/`, plus retained form routes | Integrate through documented public endpoints |
| Participant submissions | Optional captured form/credential values and network-derived details | Closed event/presence schema; old opaque details and locations removed from active data | Redesign pages and consumers that expect captured values |
| Campaign origins | Campaign URL without Ruphish's ownership-verification workflow | HTTPS DNS origin with control, routing, and certificate verification | Verify an origin before launch or continued use of supported migrated links |
| Database engines | SQLite and MySQL | SQLite, MySQL, and PostgreSQL with distinct upgrade contracts | Select the provider explicitly; do not assume a cross-engine importer |
| Delivery recovery | Legacy queue flags and retry state | Durable attempts, generation checks, and explicit unknown outcomes | Reconcile uncertain sends before choosing a resend |
| Reporting | Legacy result state and event history | Separate observed counts, progression, filtered analysis, and defined exports | Recheck denominators and raw-versus-derived comparisons |
| Webhook consumers | Signed event payload concept | Durable attempts, stable event identity, bounded retries, no ordering guarantee | Verify exact-body signatures and deduplicate by installation/event |

Familiar wire names are not always current UI labels: `smtp` means sending profiles, `pages` means landing pages, and group `targets` are recipients. `capture_credentials` and `capture_passwords` are ignored compatibility inputs; page and effective campaign responses return both as true. The template field `envelope_sender` is a visible-sender override. These wire names remain; do not rename JSON fields to match a translated label.

## A broader administration interface

The Rust/WASM UI adds persistent campaign drafts, a preparation/review step, delivery-attempt history and reconciliation, verified domains, observed-event analysis, recipient timelines, and CSV/XLSX exports. Multiple reporting mailboxes and webhook history make integrations visible as managed resources. Start with the [user guide](../user-guide/index.md) rather than looking for an exact copy of the old screen layout.

The interface supports English and German. Account settings independently control interface language, date/time formatting locale, and IANA time zone. Preferences follow the account across devices. Changing presentation does not change stored UTC instants or move an existing campaign schedule. See [account settings](../user-guide/account.md).

## API additions and separation

The public API adds campaign drafts, preparation, a paged campaign directory, delivery resolution, domain verification, analysis/export queries, dashboard aggregates, and reporting-mailbox resources. The interface is split into `/api/`, `/auth/`, and `/internal/`: public automation, browser authentication/session data, and browser-specific operations respectively. Legacy browser form paths also remain. The [API overview](../api/index.md) explains which surface to use.

New revision and generation checks prevent stale edits or delivery decisions from overwriting newer state. Use the endpoint-specific contract instead of assuming all resources accept the same update envelope.

## Participant data and page behavior

Legacy capture flags no longer authorize collecting entered values. Submission events and password presence are always recorded. The served page restricts authored active content and uses a closed event schema; importing a website does not preserve all scripts, forms, or externally hosted assets. Read [participant data](../user-guide/participant-data.md), including its verification limits.

An event is an observation, not proof that a particular person acted. New reporting distinguishes observed event totals, unique recipients, inferred progression, and delivery state. Old summary counts and new analysis counts are not interchangeable.

## Operations and recovery

Campaign origins require DNS, routing, and TLS verification. SMTP trust and authentication checks may reject previously accepted insecure configurations. A timeout after SMTP submission can become an explicit unknown outcome requiring a decision; Ruphish does not promise exactly-once mail delivery.

New passwords use Argon2id while supported legacy hashes remain usable. Generated temporary administrator passwords appear in startup stderr, including container output, but not the application log file. Supplied passwords and API tokens are never echoed. Browser sessions, current-password changes, and API keys have separate lifecycles.

SQLite, MySQL, and PostgreSQL have distinct startup and migration contracts. PostgreSQL is an additional provider, not an automatic target for a legacy MySQL or SQLite database. Re-read [configuration](../administration/configuration.md) and [database setup](../administration/databases/index.md) before reusing deployment files.
