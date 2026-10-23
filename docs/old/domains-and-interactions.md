# Domains and public interactions

For a self-contained local DNS/TLS/inbox fixture and the manual browser/privacy checklist, see [Public-interaction acceptance](public-acceptance.md).

Create a domain from Resources → Domains. Enter its public HTTPS origin, publish the displayed DNS TXT record and point the hostname at the public listener or a trusted reverse proxy. A domain becomes available in campaign creation only after control, DNS, instance routing and HTTPS all pass. Existing campaign origins are imported as unverified; upgrading never approves an old hostname automatically.

Create/save actions are at the bottom right of the domain form, with cancellation on the left. In the domain view, use **Copy record name** and **Copy TXT value** in the right-aligned DNS record column of the Domain control row, then choose **Verify domain** after publishing them. Each copy button shows **Copied** for two seconds after success without moving or resizing the controls; clipboard failures remain visible as errors. The full TXT strings remain out of the default layout.

## Verification and delivery

The exact TXT record is `_ruphish-verify.<hostname>` with value `ruphish-verification=<random challenge>`. Keep it published for periodic checks. Each job also generates a private, short-lived routing proof served at `/.well-known/ruphish/<challenge>`; a generic page reflecting the public TXT value cannot satisfy this proof. Every resolved address must be permitted and return the correct proof over trusted HTTPS. Redirects are not followed. DNS names are canonicalized to ASCII/IDNA; wildcard names, IP literals, user information, fragments and template expressions in the hostname are rejected. A campaign can retain a path/query beneath its fixed origin.

Checks are persisted and leased for 90 seconds. A probe has a 45-second overall deadline and four-second per-address HTTP deadline, checks at most 16 addresses and accepts at most 128 response bytes. Only one verification runs per process. Revision and generation fencing discard stale results after edits, disabling, deletion or recovery. Successful evidence lasts 24 hours and automatic rechecks run every six hours with up to five minutes of scheduling jitter. Failures retry with capped exponential delays; manual requests coalesce with pending work and have a 30-second completion cooldown. Expired challenges rotate when a new check is requested. Successful periodic checks keep a published challenge current.

The latest completed check is authoritative: a failed or timed-out check immediately revokes verification, including previously successful evidence. The same readiness rule gates campaign review, creation, queue claims and the final SMTP submission boundary. Expired evidence pauses new delivery without consuming SMTP retries or deleting recipients. Expiry alone does not stop already sent links, but a failed recheck, disabled domain or completed campaign stops new public interactions. The campaign view shows when delivery is waiting for verification. SMTP already submitted before disabling cannot be recalled.

The domain status is always visible: Verified, Not verified, Expired or Failed. Enabled/disabled is a separate setting. A queued or running attempt appears above the results, with its scheduled start or elapsed running time; it disappears when the attempt finishes. The page refreshes every two seconds during active work and every ten seconds otherwise, pauses polling while hidden, and refreshes on window focus. **Refresh status** reads the current result without starting a check. **Verify domain** requests a check and is unavailable while one is already active. Refresh failures retain the last confirmed result with a stale-status warning.

The bottom history table retains the ten latest completed attempts, including Passed, Failed and Timed out outcomes, completion time, duration and an actionable explanation. Both queued/running attempts and history survive page refresh and server restart. An abandoned worker lease is recorded as a timeout before replacement work starts; late results cannot overwrite the replacement. Upgrades preserve the last known historical outcome without inventing missing start times, and invalidate old retained evidence when the last known check failed.

The status, enablement and timing fields share a white details panel matching campaign details. A failed DNS lookup can complete immediately: the 45-second probe deadline is a maximum, not a required waiting period. `domain_txt_unavailable` means the TXT lookup failed; `domain_txt_mismatch` means a TXT answer was obtained but it did not contain this domain's exact proof. Check the resolver used by the running server, not just the administrator's browser. Each installation generates its own proof and must verify routing back to itself. Repeated failures retry at up to 30-minute intervals; the six-hour cadence applies after successful verification.

Hostnames are immutable after creation. Disable a domain to stop new public handling and future delivery; delete only domains that no campaign references. One account can claim a hostname, including its different ports; another account cannot gain use merely by creating a duplicate resource. Automatic sharing or transfer is not implemented. Legacy HTTP/IP/dynamic-host campaigns retain their original URLs/history and remain unavailable for further delivery; create a replacement campaign using a verified HTTPS domain rather than rewriting old sent links.

## Installation settings

`domain_verification` in `config.json` accepts these installation-owned settings. Resource API clients cannot change them.

| Field | Default | Meaning |
| --- | --- | --- |
| `resolvers` | `[]` | System DNS resolver configuration. Alternatively supply explicit socket addresses such as `"127.0.0.1:5353"`. |
| `allowed_networks` | `[]` | Explicit private destinations permitted for verification probes, as CIDRs. Metadata/link-local and unspecified addresses remain prohibited. This is separate from legacy import/SMTP policy. |
| `trusted_ca_path` | `null` | Optional PEM CA certificate for an internal public listener. Hostname and chain validation remain enabled. Paths follow the application's working directory. |
| `trusted_proxies` | `[]` | Peers permitted to supply exactly one `X-Forwarded-Host` and `X-Forwarded-Proto: https`. The proxy must overwrite both headers and terminate participant HTTPS. Forwarded headers from other peers are ignored. |

For direct TLS, set `phish_server.use_tls` to true and supply its certificate/key paths. The certificate must cover each selected hostname; the default development certificate does not establish control of an arbitrary public domain. A SAN certificate can cover several hostnames. A reverse proxy can handle separate certificates/SNI and forward to the HTTP public listener. Built-in ACME, per-host certificate automation and DNS-provider changes are not implemented. Admin HTTP remains usable locally; campaign public origins require HTTPS.

## API

| Route | Purpose |
| --- | --- |
| `GET /api/domains/` | All owned domains, including status, enablement and active check; completed history is omitted from lists. |
| `GET /api/domains/validated/` | Only owned domains currently eligible for campaign use. |
| `POST /api/domains/` | Create `{name, origin, enabled}`; returns 201. |
| `GET /api/domains/{id}` | Read status, active check, latest ten completed checks, eligibility, revision, TXT instructions and reference count. |
| `PUT /api/domains/{id}` | Update metadata/enablement with the current `revision`; origin is immutable. |
| `POST /api/domains/{id}/verify` | Request/coalesce verification; returns 202 and the current resource. |
| `DELETE /api/domains/{id}` | Delete an unused resource with quoted revision in `If-Match`. |

Existing session/CSRF, API-key and permission rules apply. Domain request bodies are bounded to 8 KiB and reject arrays, unknown fields and attempts to write verification state. Invalid input returns 400, unauthenticated access 401, permission failures 403, absent/other-owner objects 404, duplicate/in-use resources 409, stale revisions 412 and missing deletion revision 428. Domain validation messages and verification `code` values are stable identifiers such as `domain_invalid_origin`, `domain_not_verified`, `domain_txt_mismatch`, `domain_destination_denied`, `domain_https_unavailable`, `domain_routing_mismatch` and `domain_claim_conflict`. Resolver/peer/parser diagnostics are never returned.

## Participant privacy and page behavior

Rendered pages preserve form names, IDs, autocomplete attributes, labels, inline styling and document-root presentation attributes. Go expressions execute before the final policy. Authored scripts, event handlers, frames, objects, form destinations, external images/fonts/stylesheets and automatic refresh are removed or blocked. Embedded PNG/JPEG/GIF data images work. Saved authored content is retained; these restrictions apply to the served representation. Preview uses the same sanitizer with submission tracking disabled. Review imported pages under this policy before launching a campaign.

Only the trusted collector can submit a supported form. It sends JSON containing a random `event_id` and a boolean `password_present`. It does not serialize fields, send values, record keystrokes or validate credentials. Password controls retain their identity when their display type is changed. The browser aborts an unconfirmed request after 15 seconds, including a stalled response body, and allows retry. A retry reuses the original event ID and presence boolean because the server may already have committed the first attempt. Success requires an explicit acknowledgement. Every new submit creates a fresh observation; the separate retry control resends the original observation. With no JavaScript or a failed collector, response CSP blocks native submission. Actual browser enforcement and password-manager interaction require the manual checks below.

Personalized pages use the campaign's saved recipient data and existing Go-template expressions. The request reads a consistent database snapshot, releases its connection, then builds the template context, renders URL/redirect/page expressions and sanitizes the final HTML in a bounded blocking task. A short final transaction rechecks campaign/domain availability and commits the event and recipient status together. Rendering retains its capacity permit until the blocking task ends, even if its HTTP caller is cancelled. Opens and reports do not enter the rendering queue.

The server accepts only that closed JSON object, bounded to 512 bytes. Unknown fields/type mismatches return empty 400 responses, oversized bodies 413 and form-encoded/multipart payloads 415. Event limits return 429 and unavailable persistence returns 503, without echoing submitted bytes. Submission IDs and events commit atomically. Legacy capture flags are ignored; submissions and password presence are always recorded. No configuration enables value capture. No participant IP/user-agent or arbitrary query values are added to event records. Repeated/out-of-order opens cannot regress a clicked/submitted result; reports remain independent.

Existing tracking pixels, report routes, transparency suffixes (`+` and `%2b`), robots, previews and Go-template redirects remain available. Transparency retains the legacy `server: "gophish"` protocol marker. A submission returns a closed JSON result with its resolved HTTP(S)/relative redirect; the collector performs navigation without transmitting the form. HEAD and transparency do not record interactions. Completed campaigns reject new public interaction. Opens/clicks represent requests and may come from scanners or image proxies.

Optional public files under `static/endpoint/` are snapshotted at startup and served under `/static/` for verified, enabled hosts. The server does not browse the filesystem per request, follow symlinks or list directories. Limits are 1,000 files, 8 MiB per file and 32 MiB total, with bounded nesting. Hidden files are not public. Static documents have a restrictive sandbox/CSP that blocks scripts, native forms and remote content. They are not an alternate credential-capture path. Changes require a restart. Landing-page embedded-resource restrictions still apply when a resource also exists under `/static/`.

## Manual acceptance

Use an isolated local/internal domain and synthetic form values. Keep DevTools Network open with Preserve log; no actual participant credentials are needed.

1. Create a domain. Confirm it appears in Domains but not the campaign selector. Publish an incorrect TXT value, then the displayed exact value; observe actionable check status. Confirm the intended hostname, certificate and proxy route before it becomes available.
2. Create a campaign with a verified domain and a landing form containing named username/password controls with autocomplete attributes. Confirm the domain selector and path build the expected URL, and review automatically validates eligibility.
3. Open the delivered recipient link. Check layout, labels, keyboard navigation and autofill. Type synthetic values such as `privacy-user-731` and `privacy-password-731`. Typing must produce no network request. Submit: the request body must contain only `event_id` and `password_present`, with neither canary in its URL, headers or body.
4. Repeat with an empty password, a form-associated password input outside the form, and a revealed password control. Inspect event history; every submission is recorded with the eligible password-presence boolean. No field values appear in history, APIs, logs or errors.
5. Disable JavaScript and try native submission. It must be blocked by CSP. Try imported authored script, an inline event handler, `formaction`, iframe and meta refresh; none may create an alternate transmission path. Check the console for expected blocked-content reports and verify the resulting visible page is acceptable.
6. Disconnect the network during submission, reconnect and retry. The same event ID must be reused and only one event persisted. Check successful redirect behavior without carrying form values.
7. Disable the domain: new public requests fail and remaining deliveries stay queued. Re-enable/reverify and confirm eligible work resumes. Complete the campaign and confirm subsequent clicks/submissions do not change its history.
8. Inspect Domains at narrow/wide widths: aligned copy controls, four checks, stable refresh, edit conflicts, deletion protection and campaign wait message. Manual browser, password-manager and reverse-proxy acceptance remain user-owned.
