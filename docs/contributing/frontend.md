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

## Verification

Use functional Rust/JS tests for extracted transformations and request construction, then build the WASM distribution when UI output changes. Follow [validation](testing/validation.md) for the affected targets. The user performs [manual browser review](testing/manual-ui.md); a build or Node WASM test does not establish layout, keyboard, editor, or participant-browser behavior.
