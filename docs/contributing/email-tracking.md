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

## Verification on 2026-10-31

Windows, pinned Rust 1.99.1, development builds, one Cargo job/test thread, and the canonical disposable database fixture. The website work is excluded from this change and its verification.

- `cargo test -p ruphish-domain -p ruphish-contracts -p ruphish-application -p ruphish-ui --lib --tests --features ruphish-ui/demo --locked`: 117 passed. The final UI state adjustment also passed all 11 cases with `cargo test -p ruphish-ui --lib --features demo campaign_state:: --locked`.
- `cargo test -p ruphish-adapters --test email_tracking --test resource_formats --test sqlite_upgrade --locked`: 32 passed, including every supported SQLite legacy fixture.
- `cargo test -p ruphish-adapters --test reporting --locked`: 9 passed.
- `cargo test -p ruphish-adapters --test database_providers provider_contract --locked -- --include-ignored --nocapture --test-threads=1`: SQLite, MySQL 9.7.2, and PostgreSQL 18.6 passed, including the final incomplete-draft recovery regression.
- `cargo test -p ruphish-adapters --test database_providers mysql_upgrades_every_legacy_version_preserving_identity_and_recoverable_originals --locked -- --ignored --nocapture --test-threads=1`: all 28 historical versions upgraded and reopened on MySQL 9.7.2.
- With the documented MySQL 8.4.11/PostgreSQL 17.11 fixture images, `cargo test -p ruphish-adapters --test database_providers --locked -- mysql_provider_contract postgres_provider_contract mysql_upgrades_every_legacy_version_preserving_identity_and_recoverable_originals --ignored --nocapture --test-threads=1`: 3 passed, including all 28 MySQL histories. These floor-version runs preceded the shared incomplete-draft recovery adjustment; the final recovery case was rerun on the three default providers above.
- `cargo test -p ruphish --test startup_process --locked -- campaign_process:: public_process:: --nocapture --test-threads=1`: 25 passed initially; one new fixture incorrectly compared a creation response with a complete read. After correcting that baseline, `cargo test -p ruphish --test startup_process --locked -- --exact public_process::verified_domain_to_campaign_to_private_events_survives_restart_and_rejects_bypasses --nocapture --test-threads=1` passed. All 26 selected scenarios therefore passed across the focused runs.
- After the incomplete-draft recovery fix, `cargo test -p ruphish --test startup_process campaign_process::campaign_draft_process:: --locked -- --nocapture --test-threads=1`: 7 passed. Process tests used the process-local Windows console correction documented below.
- `cargo clippy --workspace --all-targets --features ruphish-ui/demo --locked -- -D warnings` and `cargo clippy -p ruphish-ui --all-targets --target wasm32-unknown-unknown --locked -- -D warnings`: passed.
- `cargo test -p ruphish-ui --test '*' --target wasm32-unknown-unknown --locked`: 45 passed, including real Fetch, request cancellation, translation, form, and download scenarios.
- `trunk build --locked`: passed; generated the current development UI distribution and manifest.
- `node --test crates/ruphish-ui/web/tests/ui-interactions.artifact.mjs`: all 41 scenarios passed against that distribution, including disabled-tracking results with historical open observations.
- `node --test --test-concurrency=1 demo/tests/api.test.mjs demo/tests/sorting.test.mjs demo/tests/preview.test.mjs`: 31 passed.
- `cargo fmt --all -- --check`, Leptos formatting checks for the three changed markup files, scoped Prettier/ESLint checks for the four changed JavaScript files, and local Markdown link-target checks passed.

Manual visual review and full release acceptance were not run; the checks above verify the affected feature and its consumers.

The first provider regression used an invalid zero-limit report filter. The fixture was corrected and all provider cases were rerun successfully. Historical observations were retained; no tracking inference was made from imported HTML.

## Historical verification on 2026-10-24

Windows, pinned Rust 1.99.1 and Cargo 1.99.1; development profile, one build job/test thread, two Tokio/Rayon workers. These commands exited 0:

- `cargo test -p ruphish-adapters --test email_tracking --test resource_formats --locked`: 17 passed. The final tracker-only run after the refresh-directive edge case passed all 5 tests.
- `cargo test -p ruphish --test startup_process resource_process:: --locked -- --nocapture --test-threads=1`: 14 passed.
- `cargo test -p ruphish --test startup_process --locked -- --exact resource_process::actual_test_delivery_composes_mime_persists_preview_and_never_records_campaign_events resource_process::email_create_edit_and_preview_share_validation_without_mutating_source campaign_process::executable_launch_delivers_personalized_mail_once_and_persists_acceptance_across_restart --nocapture --test-threads=1`: all 3 final executable regressions passed.
- `cargo clippy -p ruphish-adapters --lib --test email_tracking --test resource_formats --locked -- -D warnings`: passed.
- `cargo clippy -p ruphish --bin ruphish --test startup_process --locked -- -D warnings`: passed, including the final executable test changes.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Initial tests exposed an encoded-URL duplicate-removal defect; URL comparison now decodes HTML attribute entities. A plain-text test was corrected to inspect actual MIME parts because mail-parser's convenience HTML accessor synthesizes HTML for plain text. The persistence regression now compares GET snapshots, respecting the existing null-versus-empty attachments distinction between create and read responses. These corrections were rerun successfully. Clippy's test-helper unwrap diagnostic was resolved with a narrow assertion-helper exemption; production lint rules remain unchanged.

The first resource process run failed 13 cases at Windows shutdown and one at the corrected persistence assertion. This runner inherited Windows' ignore-Ctrl+C attribute. Clearing that attribute in the launching PowerShell process with `SetConsoleCtrlHandler(NULL, FALSE)` made the unchanged SMTP/restart test pass, and subsequent Cargo process runs passed with the same launcher correction. No application signal handling, fixture shutdown assertions or timeouts were changed. See [Microsoft's console-handler contract](https://learn.microsoft.com/windows/console/setconsolectrlhandler). The process-local correction used before those commands was:

```powershell
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class TrackerTestConsole { [DllImport("kernel32.dll", SetLastError=true)] public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add); }'
if (-not [TrackerTestConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $false)) { throw 'Cannot restore console Ctrl+C handling' }
```

Root specification verification (`py -3 tools/verify_specification.py`) could not execute because Python is not installed. No browser/inbox rendering or full provider matrix was run; this change has no UI assets, schema or provider-specific SQL changes.
