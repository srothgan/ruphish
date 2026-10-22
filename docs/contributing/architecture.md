---
title: Architecture
description: Follow requests and background work through Ruphish's seven responsibility boundaries.
sidebar:
  order: 3
---

Ruphish has a native Rust server and a Leptos admin interface compiled to WASM. The UI consumes HTTP contracts; participant landing pages are server-rendered independently of the admin bundle.

## Crate boundaries

| Crate | Owns | Direct workspace dependencies |
| --- | --- | --- |
| `ruphish-domain` | Invariants, validation, permissions, state transitions, scheduling rules | None |
| `ruphish-contracts` | Browser-safe request/response types and wire serialization | Domain |
| `ruphish-application` | Use cases, transaction boundaries, storage and integration interfaces | Domain |
| `ruphish-adapters` | Databases, SMTP, IMAP, webhooks, imports, rendering, outbound policy | Domain, application |
| `ruphish-http` | Routing, session/CSRF integration, authorization boundaries, contract mapping | Application, contracts |
| `ruphish-ui` | Admin pages, client requests, incomplete edit state, presentation | Contracts, domain |
| `ruphish` | CLI/configuration, dependency wiring, listeners, workers, shutdown | Backend crates; never UI |

The workspace tests in `tests/workspace.rs` enforce allowed direct dependency edges and inherited settings. Domain rules have one owner. A different wire shape does not justify duplicating validation or serializing a secret-bearing database model into the browser.

## HTTP surfaces

The admin listener separates **`/api/`, `/auth/`, and `/internal/`**. `/api/` contains public resource and workflow endpoints, including Ruphish additions. `/auth/session` bootstraps/revalidates browser state. `/internal/` contains browser-specific account operations such as preferences and current-password changes. Legacy browser form paths remain alongside these namespaces. External clients should use the documented [public API](../api/index.md).

A normal resource write goes from HTTP decoding and identity resolution to an application use case, domain validation, and an authorized atomic provider write. The handler maps the confirmed result to a wire contract. The UI retains a separate editable draft until that response confirms the saved state.

The participant listener binds campaign requests to verified origins and campaign state. It serves constrained page content and accepts only the allowed interaction schema. Trace privacy changes through browser collection, server decoding, persistence, logs, reporting, and webhooks; HTML field-name changes alone do not establish the boundary.

## Background work

Campaign, domain-verification, webhook, and mailbox workers use persisted claims and checkpoints. Providers own atomic persistence and ownership fencing; shared orchestration performs external work outside long-lived SQL transactions. A network side effect can succeed while acknowledgement fails, so recovery must represent uncertainty instead of promising exactly-once delivery.

The executable owns explicit application state, worker lifecycle, and graceful shutdown. Configuration and database pools are not global mutable state. See [database development](database-development.md) for transaction and commit-outcome contracts.

## Time and presentation

Domain date/time code owns instant parsing, UTC serialization, IANA-zone resolution, local scheduling, and modification revisions. The UI converts instants for display using independently selected language, formatting locale, and time zone. These settings do not change stored timestamps. See [localization](localization.md).

The workspace defaults to native members. WASM compilation and browser tests are separate checks; native success does not establish browser compatibility. Shared UI components own repeated management presentation, while feature modules retain their workflows and API state.
