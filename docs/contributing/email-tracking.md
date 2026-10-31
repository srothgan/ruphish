---
title: Email tracking composition
description: Shared rendered-email validation and automatic open-tracking insertion.
---

HTML email tracking belongs to the shared SMTP composer, used by campaign delivery and test sends. The immutable campaign policy owns `track_opens`, defaulting to `true` for new campaigns and Gophish conversions. Operators choose **Track email opens** during campaign setup; they do not need to insert or maintain a tracker in the authored HTML. Plain-text-only messages remain plain text and have no image-based open tracking.

## One validation authority

`crates/ruphish-adapters/src/email_html.rs` owns rendered-email validation and HTML5 document normalization. Resource creation and editing invoke it after rendering with the existing sample recipient. Composition invokes the same function after rendering the actual recipient. The final check is necessary because Go-template branches and values can differ per recipient. Neither the browser nor individual persistence providers implement a second policy.

Stored source is preserved: validation does not rewrite Go expressions, add document wrappers to saved HTML, or insert tracking markup. Rendered fragments and optional end tags are normalized into an explicit HTML document with a body. Styles, tables, ordinary images and Outlook conditional comments are retained. Validation rejects script, embedded browsing/plugin content, forms, base elements, template elements, refresh directives, event-handler attributes, srcdoc and executable JavaScript/VBScript attribute URLs. Conditional-comment contents receive the same validation, with bounded nesting. This is a defined email-content policy, not certification that arbitrary HTML/CSS is safe in every mail client. Administrative previews retain their existing sandbox and network-blocking CSP.

## Insertion and compatibility

After rendering and normalization, the composer removes existing image elements targeting this recipient's tracking URL, including images produced by legacy `{{.Tracker}}` and `{{.TrackingURL}}` expressions and images inside Outlook conditional comments. Attribute entities are decoded before comparison. A URL fragment does not distinguish tracking requests. When `track_opens` is enabled, the composer then appends one hidden 1×1 image as the last body element. The URL is attribute-escaped. Normal email images are not removed.

Go-template syntax and legacy tracking fields remain available. Authored legacy tracker placement and presentation no longer control the managed image. Tracking expressed through other mechanisms, such as arbitrary CSS background URLs, is not an image-element compatibility contract. Existing source is not migrated or destructively stripped. The campaign setting applies even when HTML previously omitted a tracker. Gophish stores its checkbox choice in HTML rather than a separate column; conversion deliberately defaults the new Boolean to `true` regardless of that markup.

Draft previews and campaign email reviews remove the rendered tracking image instead of requesting it or showing a broken-image indicator. The source editor can still show legacy expressions already present in imported source; new templates need none. No tracking state or catalog relationship is stored on a template. Campaign test sends use the reviewed setting; standalone template/profile test sends retain enabled tracking. The public pixel endpoint returns its usual image for an eligible disabled campaign but atomically skips the open event, result mutation, and webhook work. Mail clients can block image loading, so an absent open event is not proof that a message was unread.

## Functional coverage

`crates/ruphish-adapters/tests/email_tracking.rs` exercises real MIME composition and rendering for full documents, fragments, malformed/optional endings, conditional Go expressions, Outlook comments, repeated legacy images, encoded URLs, raw-text elements, ordinary images, plain-text-only messages, preview omission, forbidden content, recipient-dependent rejection and document limits. Existing inherited MIME tests continue asserting recipient content, headers, alternatives and attachments with automatic tracking as a deliberate output difference.

Executable cases in `tests/resource_process.rs` cover creation and editing rejection without partial persistence, unchanged authored source, tracker-free preview responses, and the tracking URL in MIME actually received by loopback SMTP. They request that URL and verify the PNG response. Recipient-dependent invalid HTML is rejected before any SMTP connection or preview persistence. The queued campaign-delivery case in `tests/campaign_process.rs` verifies managed tracking from an authored fragment with no tracker, personalized MIME, idempotent launch and acceptance across restart.

`crates/ruphish-adapters/tests/support/open_tracking.rs` runs enabled/disabled campaigns through all three real database providers, queued MIME composition, repeated public pixel requests, independent clicks/submissions/reports, and CSV/XLSX exports. Draft launch rejects a changed tracking choice after review. Failed or unresolved page defaults remain saveable without losing an explicit tracking opt-out; launch stays blocked until the destination is resolved. The HTTP campaign test covers disabled sample delivery and copy preservation; `tests/public_process.rs` checks that query parameters cannot enable tracking for an opted-out campaign and that the setting survives restart. SQLite and MySQL historical conversion tests verify the enabled default even without authored tracker markup.

The built-UI `results-untracked` scenario in `crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs` checks the translated summary, compact recipient cells, and hidden open chart series even when the raw snapshot contains historical open observations.

## Validation

Use the focused tests above when changing tracking behavior and follow [validation](testing/validation.md) for applicable provider, executable, WASM, and built-UI checks. Migration changes also require the relevant [database-provider histories](testing/database-providers.md). Keep per-run commands and results in review notes rather than treating an old test run as evidence for a newer revision.

The [manual UI checks](testing/manual-ui.md#first-release-feature-boundaries) cover tracking choices, email rendering, disabled metrics, and participant behavior. Protocol tests and a successful build do not establish inbox rendering, browser behavior, or full release acceptance.
