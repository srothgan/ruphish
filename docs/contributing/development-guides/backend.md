---
title: Backend development
description: Change HTTP contracts, application behavior, rendering, and background work at their existing owners.
sidebar:
  order: 220
---

Start with the [architecture](architecture.md) and trace an existing operation similar to the one you are changing. Resource edits provide a useful example: `crates/ruphish-http/src/resources.rs` maps requests, the contracts crate defines wire types, application/domain types describe writable input and validation, and adapter providers implement atomic persistence.

## Change a request or response

1. Identify the authoritative contract in `crates/ruphish-contracts/src/`. Resource requests live under `resources::request`, integration configuration under `integrations::request`, and campaign operations under `campaign` and `campaign::request`.
2. Update the domain/application representation where behavior changes. A writable input must not accept persisted IDs, attribution, revisions, or other server-managed response metadata as ordinary fields.
3. Update HTTP decoding and mapping. Keep handlers thin: they should not recreate a transaction through several independent writes or serialize a database model containing secrets.
4. Update the provider operation and all affected browser callers. Keep absent, empty, cleared, and retained secrets distinct according to the endpoint's contract.
5. Add focused serialization, rule, HTTP, and persistence checks where those boundaries change. Update the [HTTP reference](../../api/index.md) with valid requests, responses, attributes, errors, and retry behavior.

The contracts crate is the executable wire authority. The HTTP documentation describes it; it is not a second handwritten schema. Shared attachment/header values belong in `resources::fields`, not separate copies in each request type.

## Preserve authorization and concurrency

Role permissions govern shared resources. Authenticate the caller, enforce the operation's permission, and recheck authorization and the revision inside the write transaction. Do not reinterpret creator attribution as private access scope. See [permissions](../../administration/team-administration/access-and-permissions.md).

Configuration revisions, worker generations, and idempotency keys solve different problems. Revisions detect stale edits; generations fence obsolete workers; operation identities reconcile retries. Never substitute one for another. Endpoint-specific `If-Match` or body requirements belong in the public API reference.

Mutations returning attributed records use `attribution::Saved<T>` to capture the record and display identities inside the transaction. Response mapping then needs no later database lookup that could fail after a successful write. Keep typed uncertain outcomes when commit acknowledgement is lost; see [database development](database-development.md#failure-and-uncertainty).

## Change rendering or participant behavior

Page rendering is owned by `ruphish-adapters::landing::render`. Live pages, functional previews, and test-send previews select an explicit rendering mode rather than each implementing their own sanitizer or collector. `ruphish-domain::landing` owns the shared browser capability policy.

Landing pages run authored content in an opaque-origin sandbox with managed forms. Learning pages follow the separate static-content sanitizer. ZIP imports are processed in memory without extracting files or fetching remote dependencies. Administrative content operations use the application content-work boundary and shared rendering admission capacity; a cancelled HTTP request must not release a permit while its blocking computation is still running.

When changing collection, test both accepted observations and rejected input. Verify that sensitive canaries do not appear in stored events, logs, responses, or exports. Browser sandbox enforcement and actual form behavior also need the [manual browser checks](../testing/manual-ui.md#participant-browser-checks).

## Email composition and open tracking

HTML email tracking belongs to the shared SMTP composer, used by campaign delivery and test sends. The immutable campaign policy owns `track_opens`, defaulting to `true` for new campaigns and Gophish conversions. Operators choose **Track email opens** during campaign setup; they do not need to insert or maintain a tracker in the authored HTML. Plain-text-only messages remain plain text and have no image-based open tracking.

### Rendered-email validation

`crates/ruphish-adapters/src/email_html.rs` owns rendered-email validation and HTML5 document normalization. Resource creation and editing invoke it after rendering with the existing sample recipient. Composition invokes the same function after rendering the actual recipient. The final check is necessary because Go-template branches and values can differ per recipient. Neither the browser nor individual persistence providers implement a second policy.

Stored source is preserved: validation does not rewrite Go expressions, add document wrappers to saved HTML, or insert tracking markup. Rendered fragments and optional end tags are normalized into an explicit HTML document with a body. Styles, tables, ordinary images and Outlook conditional comments are retained. Validation rejects script, embedded browsing/plugin content, forms, base elements, template elements, refresh directives, event-handler attributes, srcdoc and executable JavaScript/VBScript attribute URLs. Conditional-comment contents receive the same validation, with bounded nesting. This is a defined email-content policy, not certification that arbitrary HTML/CSS is safe in every mail client. Administrative previews retain their existing sandbox and network-blocking CSP.

### Tracking insertion

After rendering and normalization, the composer removes existing image elements targeting this recipient's tracking URL, including images produced by legacy `{{.Tracker}}` and `{{.TrackingURL}}` expressions and images inside Outlook conditional comments. Attribute entities are decoded before comparison. A URL fragment does not distinguish tracking requests. When `track_opens` is enabled, the composer then appends one hidden 1×1 image as the last body element. The URL is attribute-escaped. Normal email images are not removed.

Go-template syntax and legacy tracking fields remain available. Authored legacy tracker placement and presentation no longer control the managed image. Tracking expressed through other mechanisms, such as arbitrary CSS background URLs, is not an image-element compatibility contract. Existing source is not migrated or destructively stripped. The campaign setting applies even when HTML previously omitted a tracker. Gophish stores its checkbox choice in HTML rather than a separate column; conversion deliberately defaults the new Boolean to `true` regardless of that markup.

Draft previews and campaign email reviews remove the rendered tracking image instead of requesting it or showing a broken-image indicator. The source editor can still show legacy expressions already present in imported source; new templates need none. No tracking state or catalog relationship is stored on a template. Campaign test sends use the reviewed setting; standalone template/profile test sends retain enabled tracking. The public pixel endpoint returns its usual image for an eligible disabled campaign but atomically skips the open event, result mutation, and webhook work. Mail clients can block image loading, so an absent open event is not proof that a message was unread.

Focused coverage lives in `crates/ruphish-adapters/tests/email_tracking.rs`, `tests/resource_process.rs`, `tests/campaign_process.rs`, and the shared provider scenario `crates/ruphish-adapters/tests/support/open_tracking.rs`. Check actual MIME received by controlled SMTP, not only an HTML helper's output. UI behavior for disabled tracking also has a `results-untracked` mounted scenario.

## Change a worker

Workers perform external work outside long-lived SQL transactions. Persist claims, fencing generations, and checkpoints through application-owned interfaces, then record outcomes through the provider's atomic operations.

Trace a change through acquisition, cancellation, timeout, retry, competing processes, shutdown, and restart. An external side effect may have succeeded even if the local result was lost. Do not report every transport failure as a definite rollback, automatically retry a possibly accepted send, or claim exactly-once delivery.

## Verify the change

Start with a regression for the affected guarantee. Add consumer checks when contracts or shared rules change, real-provider cases for SQL/locking changes, and executable tests for lifecycle or protocol changes. [Validation commands](../testing/validation.md) provides the direct commands; [database-provider tests](../testing/database-providers.md) covers opt-in server cases.
