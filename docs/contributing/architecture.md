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

The admin listener separates **`/api/`, `/auth/`, and `/internal/`**. `/api/` contains public resource and workflow endpoints, including Ruphish additions. `/auth/session` bootstraps/revalidates browser state. `/internal/` contains browser-specific account operations such as preferences and current-password changes. Session actions are under `/auth/`; browser pages have independent URLs and do not accept mutation requests. Deployment probes `/health/live` and `/health/ready` are an explicit exception to the application service namespaces and remain outside session middleware. External clients should use the documented [public API](../api/index.md).

A normal resource write goes from HTTP decoding and identity resolution to an application use case, domain validation, and an authorized atomic provider write. The handler maps the confirmed result to a wire contract. The UI retains a separate editable draft until that response confirms the saved state.

The participant listener binds campaign requests to verified origins and campaign state. Authored landing pages execute in an opaque-origin sandbox with managed native forms; static learning pages retain their sanitizer. `ruphish-domain::landing` owns the browser capability policy shared by live responses and functional preview frames. Public submission responses support credential-free CORS from the opaque origin, while administrative authorization, CSRF/origin checks and credential handling remain independent. The built-in collector and event endpoint accept only the closed observation schema. Trace privacy changes through collection, server decoding, persistence, logs, reporting and webhooks; sandboxing does not prevent arbitrary authored code from reading its own controls.

ZIP landing imports run in memory without extraction or outbound fetches. Local resources become embedded HTML/CSS dependencies under document limits, so existing page persistence, copying, export and deletion own their lifecycle across all database providers. See [landing-page authoring](../user-guide/landing-pages.md) for import limits and custom collector integration, and [manual browser checks](testing/manual-ui.md) for enforcement that source/protocol tests cannot establish.

## Background work

Campaign, domain-verification, webhook, and mailbox workers use persisted claims and checkpoints. Providers own atomic persistence and ownership fencing; shared orchestration performs external work outside long-lived SQL transactions. A network side effect can succeed while acknowledgement fails, so recovery must represent uncertainty instead of promising exactly-once delivery.

The executable owns explicit application state, worker lifecycle, and graceful shutdown. Configuration and database pools are not global mutable state. See [database development](database-development.md) for transaction and commit-outcome contracts.

## Time and presentation

Domain date/time code owns instant parsing, UTC serialization, IANA-zone resolution, local scheduling, and modification revisions. The UI converts instants for display using independently selected language, formatting locale, and time zone. These settings do not change stored timestamps. See [localization](localization.md).

The workspace defaults to native members. WASM compilation and browser tests are separate checks; native success does not establish browser compatibility. Shared UI components own repeated management presentation, while feature modules retain their workflows and API state.
