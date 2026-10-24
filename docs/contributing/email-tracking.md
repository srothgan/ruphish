---
title: Email tracking composition
description: Shared rendered-email validation and automatic open-tracking insertion.
---

HTML email tracking belongs to the shared SMTP composer, used by campaign delivery and test sends. Operators do not need to insert or maintain a tracker in the authored HTML. Plain-text-only messages remain plain text and have no image-based open tracking.

## One validation authority

`crates/ruphish-adapters/src/email_html.rs` owns rendered-email validation and HTML5 document normalization. Resource creation and editing invoke it after rendering with the existing sample recipient. Composition invokes the same function after rendering the actual recipient. The final check is necessary because Go-template branches and values can differ per recipient. Neither the browser nor individual persistence providers implement a second policy.

Stored source is preserved: validation does not rewrite Go expressions, add document wrappers to saved HTML, or insert tracking markup. Rendered fragments and optional end tags are normalized into an explicit HTML document with a body. Styles, tables, ordinary images and Outlook conditional comments are retained. Validation rejects script, embedded browsing/plugin content, forms, base elements, template elements, refresh directives, event-handler attributes, srcdoc and executable JavaScript/VBScript attribute URLs. Conditional-comment contents receive the same validation, with bounded nesting. This is a defined email-content policy, not certification that arbitrary HTML/CSS is safe in every mail client. Administrative previews retain their existing sandbox and network-blocking CSP.

## Insertion and compatibility

After rendering and normalization, the composer removes existing image elements targeting this recipient's tracking URL, including images produced by legacy `{{.Tracker}}` and `{{.TrackingURL}}` expressions and images inside Outlook conditional comments. Attribute entities are decoded before comparison. A URL fragment does not distinguish tracking requests. The composer then appends one hidden 1×1 image as the last body element. The URL is attribute-escaped. Normal email images are not removed.

Go-template syntax and legacy tracking fields remain available. Authored legacy tracker placement and presentation no longer control the managed image. Tracking expressed through other mechanisms, such as arbitrary CSS background URLs, is not an image-element compatibility contract. Existing source is not migrated or destructively stripped. Automatic tracking applies to HTML templates that previously omitted a tracker; there is no opt-out checkbox in this flow.

Draft previews and campaign email reviews remove the rendered tracking image instead of requesting it or showing a broken-image indicator. The source editor can still show legacy expressions already present in imported source; new templates need none. No tracking state or catalog relationship is stored on a template. Mail clients can block image loading, so an absent open event is not proof that a message was unread.

## Functional coverage

`crates/ruphish-adapters/tests/email_tracking.rs` exercises real MIME composition and rendering for full documents, fragments, malformed/optional endings, conditional Go expressions, Outlook comments, repeated legacy images, encoded URLs, raw-text elements, ordinary images, plain-text-only messages, preview omission, forbidden content, recipient-dependent rejection and document limits. Existing inherited MIME tests continue asserting recipient content, headers, alternatives and attachments with automatic tracking as a deliberate output difference.

Executable cases in `tests/resource_process.rs` cover creation and editing rejection without partial persistence, unchanged authored source, tracker-free preview responses, and the tracking URL in MIME actually received by loopback SMTP. They request that URL and verify the PNG response. Recipient-dependent invalid HTML is rejected before any SMTP connection or preview persistence. The queued campaign-delivery case in `tests/campaign_process.rs` verifies managed tracking from an authored fragment with no tracker, personalized MIME, idempotent launch and acceptance across restart.

## Verification on 2026-10-24

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
