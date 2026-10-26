---
title: Public demo
description: Build and maintain the static administration demo and its shared synthetic dataset.
---

The public demo uses the real Leptos administration UI with one extended synthetic dataset and simulated writes. The intended address is `https://demo.ruphish.com`. The Astro website remains a separate application at `https://www.ruphish.com`. Vercel project creation, deployment, and domain setup are left to the user.

## Visitor journey

1. Open the existing login screen, which displays username `demo` and password `demodemo`.
2. Sign in and browse the populated dashboard, campaigns, recipient activity, groups, templates, landing and learning pages, sending profiles, domains, users, webhooks, reporting mailboxes, and settings.
3. Create, edit, or delete a resource. Existing client validation runs. The local transport simulates the acknowledgement, and a bottom-right notification explains that nothing was saved.
4. Creation returns an existing sample ID. Existing resource navigation opens that sample detail page; user management retains its normal return to the list. New campaign drafts use the designated sample draft UUID, and simulated launch returns a sample campaign ID.
5. Refetch or reload and see the original sample data. Export synthetic results as CSV, JSON, or XLSX. Sign out through the normal account flow.

Preview, import, SMTP, domain-verification, webhook, and mailbox actions return explicitly simulated sample responses. They do not render submitted templates through a backend, fetch submitted URLs, deliver messages, or contact integrations. A successful simulation does not assert that a real backend would accept the submitted content.

## Build mode and production behavior

The only mode flag is the opt-in Cargo feature `demo` on `ruphish-ui`. No flag means the normal application; no separate `production` flag is needed. Hostname, query parameters, and Vercel environment names cannot switch an already built artifact into demo mode.

`browser_http.rs` selects either the normal Fetch transport or the generated demo bundle at compile time. Both use the existing request lifecycle, including cancellation, deadlines, response decoding, and downloads. The demo dispatcher returns standard `Response` objects and has no network fallback. No server handlers, persistence rules, or production authentication behavior change.

The installation-capability flags in the session remain separate from the build mode. The browser demo and recorder seed both take these flags from `features` in `assets/website-demo/data.json`, enabling the same navigation. The recorder applies them after inserting the sample resources because its initial migrations ran on an empty database. Individual recorder domains, webhooks, and mailboxes remain inactive; the browser demo simulates their behavior without workers.

Normal production tests retain their assertions and run without `demo`. Dedicated demo tests verify the simulation separately. Do not execute production Fetch tests against the fake transport or reinterpret their expectations as demo assertions.

## One dataset

[`assets/website-demo/data.json`](../../assets/website-demo/data.json) holds scenario parameters. [`dataset.mjs`](../../assets/website-demo/dataset.mjs) materializes their relationships and repeated recipients/events. This fully replaces the old screenshot dataset: there is no baseline variant, expanded variant, or upgrade path.

| Inventory | Sample content |
| --- | --- |
| Users | Six accounts: demo administrator, another administrator, standard users, locked account, password-setup account |
| Recipients and groups | 617 reusable identities, four department groups, overlapping new-starter group, empty group, twelve failed-delivery recipients |
| Content | Four email templates, two landing pages, three learning pages |
| Delivery configuration | Two sending profiles and three domains illustrating verified, pending, and disabled states |
| Campaigns | Two completed campaigns, one active campaign with twelve delivery failures, one scheduled campaign, two editable sample drafts |
| Integrations | Three webhooks and two reporting mailboxes with synthetic success/failure histories |
| Reporting | Consistent campaign/recipient references, delivery attempts, presence-only events, dashboard totals, analysis, and downloads |

Dates are relative to an anchor fixed for the browser tab. The screenshot recorder anchors dates at recording time. Both consumers receive the same scenario. The SQL consumer uses current installed schemas and forces outbound integrations and domains off; the recorder retains its randomly generated password and isolated container. The browser login credentials are never installed as a server password.

The recorder now checks four campaigns, six groups, two mailboxes, twelve delivery failures, and 937 delivered messages in the last 30 days. Screenshot regeneration is a separate manual asset-authoring action; existing PNGs are not overwritten by implementation or tests.

The configured content comes directly from the real starter files: Microsoft shared document, Google account security, Docusign signature request, and Workday employee document emails; Microsoft side-panel and Google Workspace landing pages; and SLAM, Stop/Challenge/Protect, and Recognize/Resist/Report/Delete learning pages. The demo starter picker exposes this same selected subset, including supported prefill and company-logo options. `starter-content.mjs` reads the authored files for Node consumers; `build-demo.mjs` substitutes those static exports into the browser bundle and retains the source license/provenance notices. Node filesystem APIs and backend code never enter the browser bundle.

## State, redirects, and feedback

Only the synthetic actor ID and date anchor enter namespaced `sessionStorage`. Passwords and edited resources are never stored. A full-page login redirect or refresh therefore retains the demo session without retaining submitted content. Unavailable storage produces an explicit login error. Login `next` destinations are restricted to supported local paths.

All resource reads are projections of the shared dataset. Mutation responses use existing sample IDs so the next read succeeds. Existing query owners already invalidate and refetch after confirmed changes/deletion; no second cache or demo-specific cache removal is needed. Resource data therefore settles back to the unchanged fixture.

The demo banner is a fixed overlay in front of the application, with an accessible × dismiss button. It takes no layout space, so the sidebar retains its full viewport height. Dismissal lasts across client-side navigation; reloading restores the notice. The shared toast host also survives client-side navigation. It uses a polite live region, a dismiss control, and a ten-second timeout. Notices distinguish creation, edits, deletion, launch, and external-service actions. Existing inline server-success messages use simulation wording in demo builds. Clipboard operations and file downloads retain their normal completion messages because those browser actions actually occur.

Campaign drafts remain an intentionally stateless demonstration: saving a new draft routes to the sample draft, and reopening reads its original configuration. Changes are not accumulated into an in-memory mini-backend. Settings and preferences similarly return the original fixture values; simulated password/API-key changes do not alter the public credentials.

## Code ownership

- `assets/website-demo/`: the single scenario source, materializer, SQL seeder, and existing screenshot recorder.
- `crates/ruphish-ui/web/demo-api.mjs`: explicit demo endpoints, read projections, and nonpersistent responses.
- `crates/ruphish-ui/web/demo-http.mjs` and `build-demo.mjs`: browser-only request executor and generated bundle.
- `crates/ruphish-ui/web/demo-export.mjs`: synthetic XLSX workbook serialization.
- `crates/ruphish-ui/src/shared/components/toasts.rs`: shared demo notifications; feature modules announce success only after their normal response validation.
- `crates/ruphish-ui/src/account.rs`, locale files, and shared CSS: demo notice, credentials, accessible feedback, and translations.
- `demo/tests/` and `crates/ruphish-ui/tests/demo_contracts.rs`: independent demo functionality, real DTO decoding, static routing, and installed SQLite seed/reopen checks.
- `.github/workflows/validation.yml`: separate normal and demo jobs, Cargo caches, and distribution artifacts. Shared dataset changes select the product checks through `.github/paths.yml`.

## Build and validation

See [local demo instructions](../../demo/README.md) for building and previewing the artifact and [the canonical validation reference](testing/validation.md#public-demo) for exact test commands. The normal build remains `trunk build --locked`. The demo additionally bundles its transport and builds with `--features demo`.

Both local modes use Trunk's existing `dist/.stage` staging and final `dist/admin` directory. Build sequentially and rebuild the intended mode before consuming this directory. CI uses separate jobs and artifact names so a demo artifact cannot satisfy the normal product asset cache.

## Manual acceptance

Local validation covers the changed guarantees; the CI matrix remains the broader backstop. Deployment and manual browser review remain separate from passing compilation and protocol tests.

The user should review: wrong and correct login, populated dashboard, campaign analysis and recipient history, create/edit/delete with notifications and original data on reload, domain/integration sample actions, user impersonation and return, campaign draft/review/launch, CSV/JSON/XLSX downloads, nested-page refresh, clipboard feedback, logout, mobile notification placement, and editor interaction. No browser or screenshot automation is added for that visual review.
