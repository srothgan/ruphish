---
title: Public demo
description: Build the static demo and maintain its simulated responses and shared synthetic dataset.
sidebar:
  order: 290
---

The public demo uses the real Leptos administration UI with one extended synthetic dataset and simulated writes at `https://demo.ruphish.com`. The Astro website remains a separate application at `https://www.ruphish.com`. GitHub Actions prepares the demo artifact. [Demo deployment](../maintenance/demo-deployment.md) describes the separate opt-in publication workflow.

## Build and preview locally

Use the product tools from [development setup](../development-setup/index.md). From the repository root:

```sh
npm ci --prefix crates/ruphish-ui/web
node crates/ruphish-ui/web/build-demo.mjs
trunk build --locked --features demo crates/ruphish-ui/web/generated/index.html
node demo/serve.mjs
```

Open **http://127.0.0.1:8080** and sign in with **demo / demodemo**. Stop the preview with Ctrl+C. Both normal and demo builds publish to `dist/admin/`; rebuild with `trunk build --locked` before running the real application from that directory.

Campaign drafts persist in memory across client-side navigation, including Save, Back, Continue, reopening, and deletion. Reloading restores the sample dataset. Other resource writes and external-service actions are simulated. **Simulate launch** opens a labeled sample campaign and sends no mail. The email review displays the selected source with template expressions unfilled; it does not render through a backend.

CSV import review runs locally, and exports use the synthetic dataset. ZIP bundle upload requires the real application. Simulated responses do not prove a real server would accept the submitted values or that SMTP, domains, webhooks, or mailboxes are configured.

## Build mode and production behavior

The only mode flag is the opt-in Cargo feature `demo` on `ruphish-ui`. No flag means the normal application; no separate `production` flag is needed. Hostname, query parameters, and Vercel environment names cannot switch an already built artifact into demo mode.

`browser_http.rs` selects either the normal Fetch transport or the generated demo bundle at compile time. Both use the existing request lifecycle, including cancellation, deadlines, response decoding, and downloads. The demo dispatcher returns standard `Response` objects and has no network fallback. No server handlers, persistence rules, or production authentication behavior change.

The installation-capability flags in the session remain separate from the build mode. The browser demo and recorder seed both take these flags from `features` in `assets/website-demo/data.json`, enabling the same navigation. The recorder applies them after inserting the sample resources because its initial migrations ran on an empty database. Individual recorder domains, webhooks, and mailboxes remain inactive; the browser demo simulates their behavior without workers.

Normal production tests retain their assertions and run without `demo`. Dedicated demo tests verify the simulation separately. Do not execute production Fetch tests against the fake transport or reinterpret their expectations as demo assertions.

## One dataset

[`assets/website-demo/data.json`](../../../assets/website-demo/data.json) holds scenario parameters. [`dataset.mjs`](../../../assets/website-demo/dataset.mjs) materializes their relationships and repeated recipients/events. Use this source for both the browser demo and screenshot data instead of introducing another sample dataset.

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

The dataset contains four campaigns, six groups, two mailboxes, twelve delivery failures, and 937 delivered messages in the last 30 days. Screenshot regeneration is a separate manual asset-authoring action; existing PNGs are not overwritten by implementation or tests.

### Campaign timing

[`timing.mjs`](../../../assets/website-demo/timing.mjs) owns the synthetic calendar, random streams, recipient selection, and event history. These parameters illustrate activity; they are not measured industry benchmarks.

| Behavior | Synthetic rule |
| --- | --- |
| Working calendar | Monday-Friday, 09:00-17:00 UTC; no public holidays |
| Launch date | Scenario offset/time, with past weekend launches moved back to Friday and future weekend launches forward to Monday |
| Sending windows | Baseline 45 minutes; Benefits 360; Delivery 180; upcoming 30; SMTP acceptance one second after each scheduled send |
| Tracking closure | Seven calendar days after launch, or ten for Benefits; weekend closures move forward to Monday at 17:00 UTC |
| First engagement | Lognormal working-time delay, median 45 minutes, sigma 1.8; the working clock pauses overnight and on weekends |
| Follow-up actions | Elapsed-time medians: click 90 seconds, submission 45 seconds, report three minutes without a click or fifteen after a risky action |
| Repeated observations | At most three additional opens, two clicks, and one submission; reports occur once |

Clickers are selected from openers and submitters from clickers. About 20% of clickers also report, bounded by the reporting target; remaining reports come from non-clickers. Risky repeated actions stop before reporting, while later email reads remain possible. Repeated observations count as separate timeline/export rows but once per recipient in category summaries.

The generator constructs planned history before hiding future events at the snapshot. Configured engagement totals are eventual unique-recipient targets, so active campaigns may show fewer. Required actions are sampled within campaign closure; bounded sampling failure throws rather than inventing a timestamp. Seeds and per-kind event slots keep prior observations stable when adding later history.

The shared closure populates the SQL completion state and demo edit restrictions. Future closures are scenario plans, not an automatic-completion feature in the product. Keep the browser and SQL consumers on this same model; verify changed timing through the dataset tests and disposable seed test.

The demo starter picker exposes the complete shared catalog, including every supported prefill and company-logo option and the embedded controllers for interactive pages. `crates/ruphish-http/starter-templates/catalog.json` owns entry metadata and content paths for both the native server and the demo. The Node adapter `crates/ruphish-http/src/starter_templates/catalog.mjs` reads those files; `build-demo.mjs` embeds its static catalog export and retains the source license/provenance notices. Node filesystem APIs and backend code never enter the browser bundle. The executable catalog test compares every demo entry with the real native API.

The sample campaigns preload four emails, two landing pages and three learning pages by catalog ID in `assets/website-demo/data.json`; they do not define or restrict the picker. Their resource names and content come from the catalog. The configured examples remain Microsoft shared document, Google account security, Docusign signature request and Workday employee document emails; Microsoft side-panel and Google Workspace landing pages; and the three learning pages. Editing a sample resource does not change the immutable catalog.

## State, redirects, and feedback

Only the synthetic actor ID and date anchor enter namespaced `sessionStorage`. Passwords and edited resources never enter browser storage. Campaign drafts live only in the current demo API instance's memory. A full-page login redirect or refresh therefore retains the demo session without retaining submitted content. Unavailable storage produces an explicit login error. Login `next` destinations are restricted to supported local paths.

Campaign draft reads and directory rows come from one in-memory draft store initialized with copies of the shared samples. Other resource reads are projections of the shared dataset, and their mutation responses use existing sample IDs so the next read succeeds. Existing query owners invalidate and refetch after confirmed changes/deletion; no second query cache or demo-specific cache removal is needed.

The demo banner is a fixed overlay in front of the application, with an accessible × dismiss button. It takes no layout space, so the sidebar retains its full viewport height. Dismissal lasts across client-side navigation; reloading restores the notice. The shared toast host also survives client-side navigation. It uses a polite live region, a dismiss control, and a ten-second timeout. Notices distinguish creation, edits, deletion, launch, and external-service actions. Existing inline server-success messages use simulation wording in demo builds. Clipboard operations and file downloads retain their normal completion messages because those browser actions actually occur.

Temporary campaign drafts preserve the normal UI save-and-redirect behavior without changing production persistence or navigation. The demo transport retains configuration, phase and revision, rejects stale writes, acknowledges exact save/launch retries, removes deleted drafts from reads and lists, and fences launched drafts against further editing. Reloading recreates the store from the untouched samples. Phase transitions do not emit save-success toasts; explicit Save, Delete, test email and Launch feedback describe their temporary or simulated effects. The simulation does not create campaign executions or generate results from the submitted draft. Profile-test attempts and the sender-test policy also live only in the current API instance: simulated checks update badges/history, repeated operation identities reconcile without duplicate attempts, and launch review applies the selected profile’s readiness. Reloading restores the sample histories and policy. Preferences still return the original fixture values; simulated password/API-key changes do not alter the public credentials.

## Code ownership

- `assets/website-demo/`: the single scenario source, materializer, SQL seeder, and existing screenshot recorder.
- `crates/ruphish-ui/web/demo-api.mjs`: explicit demo endpoints, read projections, and nonpersistent responses.
- `crates/ruphish-ui/web/demo-http.mjs` and `build-demo.mjs`: browser-only request executor and generated bundle.
- `crates/ruphish-ui/web/demo-export.mjs`: synthetic CSV/XLSX serialization with production export columns.
- `crates/ruphish-ui/web/demo-csv.mjs`: bounded, nonpersistent CSV review for the demo; `demo/recipient-csv-cases.json` supplies shared real-server/demo acceptance expectations.
- `crates/ruphish-ui/src/shared/components/toasts.rs`: shared demo notifications; feature modules announce success only after their normal response validation.
- `crates/ruphish-ui/src/account.rs`, locale files, and shared CSS: demo notice, credentials, accessible feedback, and translations.
- `demo/tests/` and `crates/ruphish-ui/tests/demo_contracts.rs`: independent demo functionality, real DTO decoding, static routing, and installed SQLite seed/reopen checks.
- `.github/workflows/demo.yml`: dedicated demo path selection, contracts, optimized build, Cargo cache, deployment artifact and gate. Normal application checks remain in `.github/workflows/validation.yml`. Shared dataset changes select the product checks through `.github/paths.yml`.

## Build and validation

See [local demo instructions](../../../demo/README.md) for building and previewing the artifact and [the canonical validation reference](../testing/validation-commands.md#public-demo) for exact test commands. The normal build remains `trunk build --locked`. The demo additionally bundles its transport and generates an HTML entry point with the static title `Ruphish Demo`, then builds that entry point with `--features demo`.

Both local modes use Trunk's existing `dist/.stage` staging and final `dist/admin` directory. Build sequentially and rebuild the intended mode before consuming this directory. CI uses separate jobs and artifact names so a demo artifact cannot satisfy the normal product asset cache.

## Verify your change

Use [demo validation](../testing/validation-commands.md#public-demo) for contracts, exports, seed, and mounted-UI checks. Keep production Fetch tests enabled and separate from simulated demo responses.

Manually review login, draft navigation and recovery, simulation notices, exports, editor behavior, narrow layouts, and refresh of nested routes. Check that reload resets temporary changes and no operation sends mail or contacts a configured integration. Screenshot recapture is a separate asset-authoring action; tests must not overwrite the existing images.
