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

The participant listener binds campaign requests to registered origins (scheme, host, and port), the current verification policy, and campaign state. Authored landing pages execute in an opaque-origin sandbox with managed native forms; static learning pages retain their sanitizer. `ruphish-domain::landing` owns the browser capability policy shared by live responses and functional preview frames. Public submission responses support credential-free CORS from the opaque origin, while administrative authorization, CSRF/origin checks and credential handling remain independent. The built-in collector and event endpoint accept only the closed observation schema. Trace privacy changes through collection, server decoding, persistence, logs, reporting and webhooks; sandboxing does not prevent arbitrary authored code from reading its own controls.

ZIP landing imports run in memory without extraction or outbound fetches. Local resources become embedded HTML/CSS dependencies under document limits, so existing page persistence, copying, export and deletion own their lifecycle across all database providers. See [landing-page authoring](../user-guide/landing-pages.md) for import limits and custom collector integration, and [manual browser checks](testing/manual-ui.md) for enforcement that source/protocol tests cannot establish.

Administrative imports, content validation, previews, campaign mail review and test-mail composition run as owned blocking work through application services. They share the installation's rendering admission capacity with public pages and stored previews. The blocking task retains its permit until computation finishes, even if the request is cancelled; an HTTP timeout does not preempt parsing. Authorization precedes administrative processing, and persistence rechecks authorization and revisions before effects.

Campaign preparation aggregates each selected group as it is resolved and stops when a new unique recipient would exceed the campaign ceiling. The prepared audience owns the retained recipient records; group selections retain only identity, name, revision and original row count for review and launch. This preserves first-occurrence precedence and duplicate/conflict reporting without retaining every group's full content. Individual group loading still belongs to its provider.

Campaign launch stores one immutable content snapshot alongside the audience and policy: authored email and attachments, sender identity/headers, landing content, and optional learning content. Queue preparation combines that content with the current SMTP transport; arming rechecks the transport revision. Campaign reads and participant rendering consume the snapshot, never fall back to mutable content. Gophish conversion captures available resources before application use, with missing resources retained explicitly. Standalone learning resources remain live; campaign learning links resolve their own frozen content on the campaign domain.

## Background work

Campaign, domain-verification, webhook, and mailbox workers use persisted claims and checkpoints. Providers own atomic persistence and ownership fencing; shared orchestration performs external work outside long-lived SQL transactions. A network side effect can succeed while acknowledgement fails, so recovery must represent uncertainty instead of promising exactly-once delivery.

The executable owns explicit application state, worker lifecycle, and graceful shutdown. Configuration and database pools are not global mutable state. See [database development](database-development.md) for transaction and commit-outcome contracts.

## Time and presentation

Domain date/time code owns instant parsing, UTC serialization, IANA-zone resolution, local scheduling, and modification revisions. The UI converts instants for display using independently selected language, formatting locale, and time zone. These settings do not change stored timestamps. See [localization](localization.md).

The workspace defaults to native members. WASM compilation and browser tests are separate checks; native success does not establish browser compatibility. Shared UI components own repeated management presentation, while feature modules retain their workflows and API state.

Application resources are installation-wide. Creator columns retain attribution but never authorize access. Management writes authenticate the caller and preserve revision and lifecycle invariants; server providers serialize short shared-resource writes before locking caller accounts. Worker leases and generations remain independent. Account deletion removes authentication and personal state while retaining shared work and a separate non-authenticating display identity. Only confirmed human saves stamp editor attribution. Storage and domain models retain account IDs; HTTP response DTOs embed safe creator/editor references and creation/last-edit timestamps. Read response mappers resolve the requested identities in one batch. Mutations return an application `attribution::Saved<T>` containing the record and its display identities captured inside the write transaction; constructing the write response needs no later database lookup. The UI displays these confirmed references directly. Account changes invalidate cached resource responses.


## Types, schemas and editable state

Each representation has one owner. SQL migrations define storage; provider schema inventories are generated verification artifacts. Domain types own business meaning and validation. Application interfaces describe operations and atomic outcomes. `ruphish-contracts` defines browser-safe wire types consumed by both the HTTP adapter and the Rust/WASM client. The API reference documents those types; it is not a second handwritten executable schema.

Resource writes and imports use `resources::request`; integration configuration uses `integrations::request`; campaign commands and queries use `campaign` and `campaign::request`. `common` holds cross-feature envelopes, timestamps and revision-only commands. Complete resource responses live in `resource_details`; other feature modules own their response projections. Identical attachment/header wire values have one definition in `resources::fields`. Different guarantees justify different representations: permissive recipient inputs accept omitted optional text, while complete recipient responses require it; a password replacement input cannot double as a credential-safe read response.

Domain `resources::Input` contains only a name and writable content. It cannot carry a persisted ID, revision, attribution, modification timestamp or learning-page public identity. The provider resolves retained credentials and identities against the record loaded under its write lock. HTTP maps contracts to these application inputs and maps authoritative results back to response DTOs. Keep provider secrets and SQL-specific row representations out of the contracts crate.

The resource editor keeps a typed confirmed response separately from its incomplete editable JSON. Its checkpoint is a request projection, so response metadata cannot become writable through importing or editing. Successful saves replace the confirmed checkpoint and its revision together, while preserving any newer local edits. Import, export, credential omission/clearing and cancel transitions use the same request types; adding a field requires updating its authoritative contract and mapping, not another client-side field blacklist.

## Concurrent management writes

Shared resource configuration uses monotonic numeric revisions independent of wall-clock timestamps. Resource updates/deletes send the quoted revision in `If-Match`; domain, integration and campaign-draft commands use their defined header or body revision contracts. Missing required revisions are rejected, stale revisions fail without effects, and successful configuration changes advance the stored revision in the same transaction as all nested content. Strict header parsing rejects wildcard, weak, repeated and malformed validators. The public [API reference](../api/index.md) owns endpoint-specific request details and error statuses.

SQLite obtains its write transaction before loading the current version. MySQL and PostgreSQL serialize short management writes and lock relevant rows before comparing versions. Comparison, child replacement, revision advancement, attribution and response-display capture all belong to that transaction. These database locks implement the atomic comparison; users still edit optimistically without holding a database lock while a form is open. Worker leases/generations, idempotency keys and immutable launched content have separate purposes and must not reuse the configuration revision as a generic concurrency token.

A conflict preserves the user's draft and requires review of current server state; the client must not silently retry with a fresh revision. A lost commit acknowledgement is still an uncertain outcome even when response mapping needs no additional read. Provider tests exercise competing writers, edit/delete races, complete child replacement and rollback; browser-state tests cover late responses and retained edits. See [database development](database-development.md) and [validation](testing/validation.md) for the corresponding boundaries and commands.
