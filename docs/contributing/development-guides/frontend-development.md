---
title: Frontend development
description: Extend the Leptos interface while preserving form state, request ownership, and shared presentation.
sidebar:
  order: 240
---

The administration interface uses Leptos, browser-safe HTTP contracts, and shared components. Treat a feature as a workflow: loading, editing, saving, conflict, recovery, and navigation all belong to the change.

## Extend an existing feature

1. Find the feature under `crates/ruphish-ui/src/` and trace its route, contract mapping, query, and form state. Resource editing uses `resources/pages.rs`, `resources/draft.rs`, `resources/queries.rs`, and `resources/inventory.rs`. Recipient-group drafts and rows live under `resources/recipients/`; campaign recipient analysis and history stay with campaign results and delivery.
2. Reuse the shared layout and controls under `shared/components/`. Keep permissions, API calls, and feature-specific fields with the feature.
3. Add or change text in both locale catalogs. Use the shared [localization](localization.md) and date/time helpers.
4. Update validation and request projection at their existing owners. A response field must not become writable just because it appears in editable JSON.
5. Select validation for the changed guarantee using the [validation guide](../testing/validation-commands.md#frontend-and-browser-code). Add a focused state/request regression when it verifies meaningful behavior. Build the normal distribution when needed for use or manual review; the user owns visual/interaction review during agent-assisted work.

## Reuse presentation and assets

Follow the [design system](design-system.md) for layouts, controls, colors, and feedback. `crates/ruphish-ui/ruphish.css` owns shared styles; `crates/ruphish-ui/theme.css` owns the palette shared with the visual editor. Extend shared components when the same need appears in multiple features. Preserve specialized editor-tab and disclosure accessibility behavior.

CodeMirror supplies source editing and Jodit supplies visual editing in an isolated frame. Build their modules and styles from the locked browser npm dependencies. Trunk invokes `crates/ruphish-ui/web/build.mjs` before compilation; run that generator explicitly before standalone WASM checks when its outputs are missing.

The completed distribution is `dist/admin/`; Trunk uses `dist/.stage/` while building. Stop the native server before replacing its distribution, rebuild, then restart. Do not hand-edit generated files or serve an incomplete staging directory.

## Give each state one owner

| State | Owner and rule |
| --- | --- |
| Confirmed record | Typed server response and its revision |
| Incomplete form | Feature/editor draft, retained through recoverable failures |
| Applied campaign filters | URL and query key |
| Unapplied filter edits | Filter form; editing does not fetch results |
| Request progress and failure | Query/request lifecycle |
| Previously displayed campaign data | Read-only presentation snapshot; never written back under a new query key |
| Login transition | Session monitor, which invalidates older session reads |

Mount forms through stable components keyed by record identity or explicit open state. Preference refreshes and background reads must not recreate the draft owner. Do not retain secrets or imported resource bodies in URLs.

Recipient-group drafts use typed, independently reactive rows and stable editor identities. Sort once on load; keep display and submission order identical afterward. Serialize at save and retain the submitted row identities to translate indexed server diagnostics. Typing must not notify the collection or unrelated rows. Shared form validation publishes changed field errors to registered controls; touched state and request generations do not broadcast to every input.

Campaign draft persistence owns creation identity and the complete confirmed checkpoint together. An unresolved creation protects navigation even if the form has never been edited. Retry the frozen creation key and body; a returned identity alone does not authorize adopting another editor's revision. Explicit reload and confirmed saves advance revision, attribution, modification time, and saved generation atomically.

A successful save updates the confirmed checkpoint and revision together while preserving newer local edits. A stale-revision failure preserves the draft for review against current server data. An uncertain result is not a confirmed save and must not promote submitted values into the inventory.

After a confirmed resource save, reconcile returned metadata into the inventory query before refreshing. Cancel obsolete reads so a pre-save response cannot restore stale metadata. Where the save response lacks a count, retain a previously confirmed count or show it as unknown until a successful summary read; do not invent a value.

## Handle reads, errors, and navigation

Use `QueryView` and the shared query boundaries. Distinguish these states:

| Situation | Expected presentation |
| --- | --- |
| No confirmed response yet | Initial loading or an explicit error with retry |
| New filter selection with previous results | Keep the context visible and show request progress |
| Successful empty result | Show the relevant empty state |
| Temporary refresh failure | Retain confirmed content with an error and retry |
| Missing record or denied access | Clear the retained data and show the applicable boundary |
| Late response for an obsolete selection | Do not replace the current view |

Campaign results mount at `/campaigns/:id/results`, delivery at `/campaigns/:id/delivery`, and creation at `/campaigns/new`. `Section::landing` chooses the initial section from lifecycle state. The bare campaign URL resolves that destination before mounting section content; avoid two route owners for the same results view.

Campaign presentation snapshots belong to one campaign view. They may keep old content visible while a new selection loads, but cannot make exports ready for that new selection. Completed campaigns stop automatic polling; explicit filter changes and retries still work. Do not restart a pending request on every poll tick.

Login follows the session monitor's transition and shared destination policy. An obsolete refresh must not replace an in-progress login. Failed login retains the form; confirmed login replaces its history entry. Keep authentication-page loops and required password setup within that existing destination policy.

Use the shared `SessionScreen` for initial session loading, session transitions, and their failures on the net background. The account feature supplies the title, subtitle, loading/failure state, and recovery callback; the screen owns only presentation and its five-second slow-loading timer. Session reads retain the shared transport deadline and can be retried after failure. Transition recovery repeats only the intended navigation, never login or logout. Initial reads and retries belong to the app owner so replacing the card cannot cancel them. Routine session checks preserve mounted content, and rejected login/logout requests keep their existing inline errors.

`shared::feedback::Message` owns operation-message dismissal and five-second success expiry, including cancellation of obsolete timers and cleanup when its owner ends. Keep it with the operation owner rather than creating another copy for a confirmation dialog. Use `FeedbackRegion` when typed feature state already owns a failure, and dismiss only its displayed diagnostic. Preserve pending requests, retries, drafts, and recovery controls independently of notification visibility. See the [feedback policy](design-system.md#state-feedback-and-accessibility).

Capture `shared::tasks::ViewTasks` at the page boundary for requests that must survive a confirmation dialog closing. Spawning a cancellable task under the dialog's owner would cancel the operation as soon as confirmation closes. Page-owned tasks still cancel when the page is removed.

## Validate forms

Domain validators own reusable rules. `form_validation.rs` projects browser values into them. Use `FormValidation`, `validation_field` for nested paths, and `ValidationAnchor` for grouped or JavaScript-owned inputs. Shared controls associate diagnostics with fields and focus the first failing control.

Show field errors on blur or an attempted action. Editing invalidates pending diagnostics so a delayed failure cannot annotate newer input. Validation reports carry paths and stable reason codes, not submitted values. Keep transport, authentication, permission, and revision failures distinct from field validation.

The server still validates permissions, current resources, revisions, and launch eligibility. Browser validation improves feedback; it does not replace those checks. Domain origins can be HTTP or HTTPS; verification eligibility is a separate rule. Existing secret inputs have endpoint-specific preserve/replace behavior, so use the contract mapping instead of a generic empty-string rule.

Campaign drafts can save incomplete configuration. Continue validates the current step and advances after a successful save. Review checks all required steps and directs invalid input to its owning step. Keep the existing operation identity when retrying an uncertain launch.

## Verify the change

Choose from [frontend validation](../testing/validation-commands.md#frontend-and-browser-code) according to the changed guarantee. Native logic tests, WASM contract tests, and [named mounted UI scenarios](../testing/validation-commands.md#mounted-ui-regressions) provide different evidence; a frontend change does not automatically need all three. A shared component alone is not a reason to run every consumer scenario. For transport changes, select checks that exercise the affected normal Fetch contract; a passing simulated demo cannot establish that contract.

Build into `dist/admin/` when the user needs to try the frontend, and reuse the completed build for any selected mounted scenarios. An isolated validation distribution does not update the normally served application. [Manual browser checks](../testing/manual-browser-checks.md) describe keyboard operation, focus, narrow layouts, editor behavior, and recovery; the user owns this review during agent-assisted work. A compiled bundle or a Node-hosted WASM test does not prove those visual and browser guarantees.
