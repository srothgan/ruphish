---
title: Frontend development
description: Extend the Leptos interface, shared controls, and browser editor without duplicating workflow state.
sidebar:
  order: 4
---

The admin UI uses Leptos and the preserved HTTP API. Its scope includes campaign drafts and review, domain verification, delivery reconciliation, analysis and exports, account preferences, multiple reporting mailboxes, and webhook history. Treat those as complete workflows, including loading, empty, dirty, stale, failed, and unauthorized states.

## Shared presentation

Use `crates/ruphish-ui/src/shared/components/` for page layouts, lists, tables, toolbars, fields, actions, dialogs, and explanatory controls. Feature modules own data, permissions, edits, and API calls. Do not recreate a generic list toolbar or empty state in each feature.

`crates/ruphish-ui/ruphish.css` owns shared styles; `theme.css` owns the palette shared with the isolated visual editor. Follow the [design system](design-system.md), especially label/control alignment, wrapped action rows, focus, and status text. Specialized editor tabs and disclosure controls retain their own accessibility semantics.

## Editor integration and assets

CodeMirror supplies source editing and Jodit supplies visual editing through focused browser-library integration. Their modules and styles are built from the locked npm dependencies, not runtime CDN downloads. The visual editor runs in an isolated frame with a shared palette.

On a fresh checkout, install browser dependencies with `npm ci --prefix crates/ruphish-ui/web`. Trunk invokes `crates/ruphish-ui/web/build.mjs` before compiling the UI, then produces the distribution manifest. For direct WASM checks without a Trunk build, run that generator first so imported modules exist.

Do not hand-edit generated JS/CSS or move staging into the served directory. The server serves the verified `dist/admin` snapshot. Rebuild and restart when validating changed distribution inputs.

## State and localization

Keep confirmed resource data separate from incomplete form state. Preserve recoverable edits after a failed save, reconcile revision conflicts explicitly, and guard navigation that would discard work. Do not store API keys, service credentials, or imported resource bodies in URLs to retain a draft.

Use shared localization and date/time helpers; avoid literal English strings or feature-specific timestamp parsers. The [localization guide](localization.md) explains translation keys, error-code mapping, and independent account preferences.

## Form validation

Domain validators own reusable rules. `form_validation.rs` projects editable browser values into those validators, while shared controls associate localized diagnostics with fields and focus the first failing control. Use `FormValidation` for a form, supply `validation_field` for nested paths, and provide a `ValidationAnchor` for grouped or JavaScript-owned inputs. Errors become visible on blur or an attempted action. Editing invalidates pending diagnostic responses; a delayed failure must not annotate newer input.

Validation reports contain field paths and stable reason codes, without input values. They are independent of `ApiError`: an API failure may carry a report, while transport, authentication, permissions and revision failures retain their separate status and recovery behavior. Never manufacture an API failure for a local rule or display arbitrary server text as a field message.

| Surface | Shared browser/server rules | Server-dependent checks |
| --- | --- | --- |
| Campaigns | Required name and selections, redirect-choice consistency, local time parsing and schedule ordering | Resource ownership/revisions, recipient resolution, domain eligibility, rendered destinations and launch transaction |
| Resources | Required resource fields, audience count, text-or-HTML email content and header control characters | Recipient/sender/SMTP parsing, template syntax, attachment decoding and limits, name conflicts and optimistic revision |
| Passwords and users | Required username/role, candidate password bounds and confirmation | Current credential, reuse, role existence and permissions |
| Domains | Name bounds and HTTPS origin syntax | Ownership, uniqueness and verification |
| Integrations | Endpoint/configuration formats, bounds and ranges | Existing-secret preservation, permissions, revisions and connection tests |
| Settings and preferences | Duration ranges/relationships, supported language, locale syntax and time zone | Authorized persistence; browser locale availability also depends on the browser's Intl support |

These rules preserve existing accepted input. For example, the password minimum is eight UTF-8 bytes with a 1,024-byte maximum; there is no additional uppercase/digit/symbol rule. Blank passwords on user edits and blank integration secrets on existing configurations preserve saved credentials. Resource name rules do not impose a new universal minimum/maximum length. Login retains required credentials and server authentication without applying the replacement-password policy.

Draft persistence permits incomplete configuration within its existing structural bounds. Continue checks readiness for the current step and saves successfully before advancing. Review checks all steps, including a draft reopened directly at Review; invalid input reveals its owning step. Server preparation and launch independently reject invalid input. A frozen launch with an uncertain result keeps its retry identity rather than being treated as a new launch.

## Verification

Use functional Rust/JS tests for extracted transformations and request construction, then build the WASM distribution when UI output changes. Follow [validation](testing/validation.md) for the affected targets. The user performs [manual browser review](testing/manual-ui.md); a build or Node WASM test does not establish layout, keyboard, editor, or participant-browser behavior.
