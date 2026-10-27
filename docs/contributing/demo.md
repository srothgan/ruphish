---
title: Public demo
description: Build and maintain the static administration demo and its shared synthetic dataset.
---

The public demo uses the real Leptos administration UI with one extended synthetic dataset and simulated writes at `https://demo.ruphish.com`. The Astro website remains a separate application at `https://www.ruphish.com`. GitHub Actions prepares the demo artifact; deployment to the separate Vercel demo project is opt-in until its protected environments and credentials are configured.

## Visitor journey

1. Open the existing login screen, which displays username `demo` and password `demodemo`.
2. Sign in and browse the populated dashboard, campaigns, recipient activity, groups, templates, landing and learning pages, sending profiles, domains, users, webhooks, reporting mailboxes, and settings.
3. Create, edit, or delete a resource. Existing client validation runs. The local transport simulates the acknowledgement, and a bottom-right notification explains that nothing was saved.
4. Creation returns an existing sample ID. Existing resource navigation opens that sample detail page; user management retains its normal return to the list. New campaign drafts use the designated sample draft UUID, and simulated launch returns a sample campaign ID.
5. Refetch or reload and see the original sample data. Export synthetic results as CSV, JSON, or XLSX. Sign out through the normal account flow.

CSV import review parses the supplied content locally, including the downloadable recipient template, mapped headers, quoted fields and row errors. It feeds the normal draft review and duplicate handling; saving still returns a simulated sample resource and never persists imported recipients. Shared acceptance cases exercise both the demo review and the real server. Other imports, previews, SMTP, domain-verification, webhook, and mailbox actions return explicitly simulated sample responses. They do not render submitted templates through a backend, fetch submitted URLs, deliver messages, or contact integrations. A successful simulation does not assert that a real backend would accept arbitrary submitted content.

Recipient CSV/XLSX exports include the production columns for separate names, position and stored send/modification dates. Exports preserve filtering across all pages, normalize stored dates to UTC, leave absent or invalid dates blank, and protect spreadsheet-sensitive values. XLSX includes a Summary sheet and the requested Recipients/Events sheets, including their headers when empty. The shared synthetic dataset supplies explicit result dates to both the demo and its SQL seeder; exports do not infer missing dates from observations.

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
- `crates/ruphish-ui/web/demo-export.mjs`: synthetic CSV/XLSX serialization with production export columns.
- `crates/ruphish-ui/web/demo-csv.mjs`: bounded, nonpersistent CSV review for the demo; `demo/recipient-csv-cases.json` supplies shared real-server/demo acceptance expectations.
- `crates/ruphish-ui/src/shared/components/toasts.rs`: shared demo notifications; feature modules announce success only after their normal response validation.
- `crates/ruphish-ui/src/account.rs`, locale files, and shared CSS: demo notice, credentials, accessible feedback, and translations.
- `demo/tests/` and `crates/ruphish-ui/tests/demo_contracts.rs`: independent demo functionality, real DTO decoding, static routing, and installed SQLite seed/reopen checks.
- `.github/workflows/demo.yml`: dedicated demo path selection, contracts, optimized build, Cargo cache, deployment artifact and gate. Normal application checks remain in `.github/workflows/validation.yml`. Shared dataset changes select the product checks through `.github/paths.yml`.

## Build and validation

See [local demo instructions](../../demo/README.md) for building and previewing the artifact and [the canonical validation reference](testing/validation.md#public-demo) for exact test commands. The normal build remains `trunk build --locked`. The demo additionally bundles its transport and builds with `--features demo`.

Both local modes use Trunk's existing `dist/.stage` staging and final `dist/admin` directory. Build sequentially and rebuild the intended mode before consuming this directory. CI uses separate jobs and artifact names so a demo artifact cannot satisfy the normal product asset cache.

## GitHub build and Vercel deployment

The `demo` group in [the shared path selection](../../.github/paths.yml) selects UI/domain/contracts, embedded starter content, demo data, manifests, the workspace lockfile, toolchain and deployment/build inputs. Backend implementation, backend tests and website/documentation-only changes do not select the demo. `Cargo.lock` deliberately remains a broad trigger: even a backend dependency update can select the demo. Fork and Dependabot PRs keep the same relevant build/test coverage. The website retains its existing Vercel path filter for `website/` and `docs/`.

[The dedicated demo workflow](../../.github/workflows/demo.yml) runs the demo contracts and browser checks, builds the optimized demo, and packages it once. Its only jobs are Demo paths, Static demo contracts and build, and Demo gate; it does not instantiate normal application jobs. The normal application keeps its separate development artifact and tests in [shared validation](../../.github/workflows/validation.yml). The deployment build uses these direct commands after dependency installation:

```sh
node crates/ruphish-ui/web/build-demo.mjs
trunk build --locked --release --features demo
node demo/vercel-output.mjs
```

The [Trunk action](../../.github/actions/trunk/action.yml) downloads the official Linux x64 binary for the version in `Trunk.toml`, verifies the committed SHA-256 checksum, and checks its reported version. When updating Trunk, review and update the checksum from the matching [official release](https://github.com/trunk-rs/trunk/releases). Other local platforms retain their normal tool installation. The [toolchain action](../../.github/actions/toolchain/action.yml) caches Cargo registry/git downloads, `target` and installed tools. Restore keys prefer matching dependencies and fall back to the same OS, architecture, toolchain and job group; Cargo decides which changed inputs require recompilation. GitHub cache eviction and branch access rules still apply, so cold builds remain possible. See [GitHub's cache reference](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching).

[`vercel-output.mjs`](../../demo/vercel-output.mjs) verifies the asset manifest and copies only the static distribution into `.vercel/output/static`, with [Build Output API v3 configuration](https://vercel.com/docs/build-output-api/configuration). It preserves JS/MJS/WASM MIME types, serves extensionless application routes through `index.html`, rejects backend namespaces and missing asset files, and marks the demo `noindex`. It refuses stale output directories, unmanifested files and altered assets. The `demo-vercel-output` artifact contains this directory, including its routing configuration, and is retained for two days. Packaging tests exercise these rules locally; hosted behavior still requires deployment verification.

The [CI workflow](../../.github/workflows/pr.yml) calls the dedicated demo workflow alongside normal source validation. As soon as the selected demo has built and validated its artifact, the deployment job can request environment approval; it does not wait for the other checks or `PR gate`. After approval, it downloads that exact run's artifact and executes `vercel deploy --prebuilt --yes` (plus `--prod` for main). Preview and production follow the same independent path. The PR gate still requires both source and demo validation, but deployment is a separate check and can succeed even if another suite fails. Nightly and release checks reuse the same demo workflow with full selection, without deployment or comments. The deploy job does not check out application code or rebuild it. [Vercel documents prebuilt uploads](https://vercel.com/docs/cli/deploy#prebuilt) as skipping its remote build. The pinned CLI version is maintained in that job; version 65.3.1 also accepts `VERCEL_TOKEN` from the environment, verified against the installed CLI implementation.

### Activation and credentials

Keep Vercel's existing Git deployment path active until the workflow has passed and its credentials are ready. The new deploy job is disabled unless the repository variable `DEMO_DEPLOY_ENABLED` is exactly `true`. No Vercel build-setting change is required for prebuilt uploads; after cutover, the old install/build commands are unused by this workflow.

Configure these GitHub repository variables and environment secrets without committing credentials:

| Location | Name | Value |
| --- | --- | --- |
| Repository variable | `VERCEL_ORG_ID` | The Vercel team ID owning the demo project |
| Repository variable | `VERCEL_DEMO_PROJECT_ID` | The `ruphish-demo` project ID, never the website project ID |
| Repository variable | `DEMO_DEPLOY_ENABLED` | `true` only when activation is ready |
| `demo-production` environment secret | `VERCEL_TOKEN` | A Vercel token authorized for the demo project's team |
| `demo-preview` environment secret | `VERCEL_TOKEN` | A Vercel token authorized for preview deployment |

Create the `demo-production` environment with deployment restricted to `main` and a required maintainer reviewer. Create `demo-preview` with a required maintainer reviewer; review the exact PR commit and workflow before releasing its secret. For this single-maintainer repository, self-review must remain allowed if the maintainer will approve their own deployments. [GitHub environment protection](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments) withholds environment secrets until approval. Tokens belong in those environments, not in general repository secrets or Dependabot secrets.

Pushes to main publish production after approval. Same-repository, non-Dependabot PRs can publish previews after approval. Fork and Dependabot PRs build and test without publishing or receiving the deployment credential; their merged changes use the normal main deployment path. A manual run of the CI workflow on main forces validation and can publish even when no input paths changed. Manual runs on other branches do not deploy. The GitHub environment's deployment URL links to the result.

For eligible PR previews, `github-actions[bot]` maintains one comment through Building when a run starts and the canonical path filter selects the demo; Awaiting approval after the demo artifact is ready; Deploying when the approved deployment job starts; and Ready with the preview URL after deployment, or Build failed/Deployment failed with workflow logs. Each update includes the source commit and run link. Reporting uses the automatic GitHub token with `pull-requests: write`. The initial reporting job also has `contents: read` to read the path filter without executing repository code. The Deploying update runs inside the protected deployment job before artifact upload; it has a one-minute timeout and a comment failure does not fail deployment. The Vercel credential remains scoped to the actual deploy step. Reporting jobs are ordered, and comment metadata rejects updates older than an already recorded run, attempt or stage. Neither building nor deployment depends on a reporting job. Cancelled runs, closed PRs and superseded PR commits are skipped; unrelated check failures do not affect demo status. Production deployments do not post PR comments.

For cutover, first verify a prebuilt preview with the manual acceptance list below. Enable the deploy job, then disconnect only the `ruphish-demo` project's Git repository integration in Vercel before merging the workflow, so the main push does not trigger a second source build. Keep the project and its existing domain; CLI uploads target it by ID. Leave `ruphish-website` connected. If publication fails or a newer unrelated push cancels an earlier CI build, rerun CI manually on main. Disabling `DEMO_DEPLOY_ENABLED` stops new GitHub publications; reconnecting the demo Git integration restores the previous Vercel build path. This repository change alone neither disconnects Vercel nor proves a hosted deployment succeeded.

Standard GitHub-hosted runner time is free for this public repository; cache and artifact storage have separate limits. Short artifact retention and separate build caches reduce repeated work but do not guarantee a particular warm-build duration. See [GitHub Actions billing](https://docs.github.com/en/actions/concepts/billing-and-usage).

## Manual acceptance

Local validation covers the changed guarantees; the CI matrix remains the broader backstop. Deployment and manual browser review remain separate from passing compilation and protocol tests.

The user should review: wrong and correct login, populated dashboard, campaign analysis and recipient history, create/edit/delete with notifications and original data on reload, domain/integration sample actions, user impersonation and return, campaign draft/review/launch, CSV/JSON/XLSX downloads, nested-page refresh, clipboard feedback, logout, mobile notification placement, and editor interaction. No browser or screenshot automation is added for that visual review.
