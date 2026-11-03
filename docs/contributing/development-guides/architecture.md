---
title: Architecture
description: Understand the application components, dependency direction, and main request and worker flows.
sidebar:
  order: 210
---

Ruphish has a native Rust server, an administration interface compiled from Leptos to WebAssembly, and a separate listener for participant pages and observations. SQLite, MySQL, or PostgreSQL stores application state. Workers deliver mail, verify domains, process reporting mailboxes, and send webhooks.

The administration browser calls HTTP endpoints; it never connects directly to a database or mail service. Participant pages are rendered by the server and do not require the administration bundle.

## Responsibility boundaries

| Crate | Owns | Direct workspace dependencies |
| --- | --- | --- |
| `ruphish-domain` | Invariants, validation, permissions, state transitions, scheduling rules | None |
| `ruphish-contracts` | Browser-safe request/response types and wire serialization | Domain |
| `ruphish-application` | Use cases, transaction boundaries, storage and integration interfaces | Domain |
| `ruphish-adapters` | Databases, SMTP, IMAP, webhooks, imports, rendering, outbound policy | Domain, application |
| `ruphish-http` | Routing, session/CSRF integration, authorization boundaries, contract mapping | Application, contracts |
| `ruphish-ui` | Admin pages, client requests, incomplete edit state, presentation | Contracts, domain |
| `ruphish` | CLI/configuration, dependency wiring, listeners, workers, shutdown | Backend crates; never UI |

`tests/workspace.rs` verifies permitted direct workspace dependencies. Keep business rules in the domain, use cases and provider interfaces in the application layer, and external-system details in adapters. The executable wires these pieces together and owns their lifetime.

## Follow a resource edit

1. The UI loads a browser-safe response and keeps the editable form separate from the confirmed record.
2. The HTTP layer authenticates the request, decodes its contract, and maps it to an application input.
3. Shared domain validation checks the input. The application/provider operation checks the caller's permissions and the current revision.
4. The provider saves the record, nested content, new revision, and attribution in one transaction.
5. HTTP maps the confirmed result to a response. The UI updates its confirmed state while preserving any newer local edits.

A stale revision fails without overwriting another person's change. A lost response can leave the client uncertain even if the database committed. These are different outcomes and require different recovery behavior. See [backend development](backend.md), [database development](database-development.md), and [frontend development](frontend.md).

## Follow a campaign

Campaign preparation resolves selected resources and recipients, validates the intended destinations, and produces review data. Launch stores the audience, policy, and immutable content snapshot. Later resource edits do not rewrite a launched campaign's email or pages.

Delivery workers claim persisted work, compose personalized messages, use the current SMTP transport, and persist the outcome. An SMTP server can accept a message before Ruphish loses the acknowledgement of its own database write; recovery must preserve this uncertainty rather than resend blindly.

Participant requests resolve a registered origin and campaign before rendering a page or accepting an observation. Event persistence and publication of related work must stay atomic. Webhook and mailbox workers similarly use durable claims/checkpoints so restart and competing processes do not erase ownership decisions.

## HTTP surfaces

| Surface | Purpose |
| --- | --- |
| `/api/` | Resource and workflow endpoints; the [HTTP reference](../../api/index.md) documents API-key integration access |
| `/auth/` | Browser authentication and session lifecycle |
| `/internal/` | Browser-only account operations, such as preferences |
| `/health/live`, `/health/ready` | Unauthenticated deployment probes |
| Participant listener | Campaign pages, learning content, and observations |

Public API contracts and browser session behavior have different consumers. Internal routes are not an additional integration API. Keep authorization and credential handling at their respective boundaries.

## Shared state and privacy

Application resources are shared across the installation according to role permissions. Historical creator columns retain attribution; they do not restrict a resource to its creator. Accounts, sessions, caller-scoped idempotency keys, and worker claims still have their own ownership rules.

The built-in participant collector accepts a closed observation schema and optional presence booleans. It does not collect entered usernames, passwords, OTPs, or arbitrary form values. Authored landing-page scripts can read their own controls, so the collector guarantee is separate from arbitrary authored code. Review privacy changes through collection, decoding, persistence, logs, exports, and webhooks.

## Find the next level of detail

| Question | Guide |
| --- | --- |
| How do I change a request, validation rule, or worker? | [Backend development](backend.md) |
| How do transactions, schema checks, and migrations work? | [Database development](database-development.md) |
| How do I preserve form state and handle late responses? | [Frontend development](frontend.md) |
| Where do translations and time conversion belong? | [Localization](localization.md) |
| Which tests cover the change? | [Testing](../testing/index.md) |
