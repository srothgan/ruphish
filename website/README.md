# Ruphish website

The standalone Astro website contains the landing page, About page, and Starlight documentation. Run these commands from `ruphish/website/` with Node.js 24.x, as declared in `package.json`.

## Local development

Install dependencies on a fresh checkout:

```sh
npm ci
```

Start the development server:

```sh
npx astro dev --host 127.0.0.1 --port 4321
```

Open <http://127.0.0.1:4321/>. Astro reloads website edits and synchronizes changes to the canonical Markdown under `../docs/`. The generated `src/content/docs/` and `public/docs-files/` directories are ignored build inputs; edit the original sources.

## Preview with documentation search

Starlight generates its Pagefind search index during the build. Build and preview to use the complete search experience locally:

```sh
npx astro build
npx astro preview --host 127.0.0.1 --port 4321
```

Stop the running server with Ctrl+C before starting another server on the same port.

## Website assets

The R-only favicon is `public/favicon.svg`. The full wordmark, its light variant, and the R icons are in `src/assets/`. The website dashboard screenshot is `public/dashboard.png`, served at `/dashboard.png` and used by both the landing page and the documentation. The isolated [dashboard recorder](../assets/website-demo/README.md) replaces `../assets/dashboard.png`; after reviewing it, manually copy that image to `public/dashboard.png` to update the website. Recording does not synchronize the website copy.

## Validation

```sh
node --test tools/ignore-build.test.mjs tools/navigation.test.mjs
node --test tools/releases.test.ts tools/installers.test.ts
npx astro check
npx astro build
node --test tools/installers.artifact.ts
```

The Vercel configuration rebuilds for changes under `website/` or `docs/`; root `assets/` changes do not rebuild or update the website until the image is copied manually. Production canonical URLs and the sitemap use `SITE_URL` when set, otherwise `VERCEL_PROJECT_PRODUCTION_URL`. A permanent redirect adds the trailing slash that `trailingSlash: 'always'` expects to page paths; `/api/` routes and paths with a file extension are excluded so installer requests and static files are served directly. The local `astro dev` server shows its own trailing-slash error instead of `404.astro` for paths without the slash.

## Native installers and release discovery

`public/install.sh` and `public/install.ps1` are the canonical installer sources. Astro copies them unchanged to `/install.sh` and `/install.ps1`; `vercel.json` supplies plain-text content types, `nosniff`, and cache revalidation. Usage, updates, downgrade confirmation, and removal are documented in the [installation guide](../docs/getting-started/installation.md). The terminal presentation takes inspiration from `claude-code-rust`; the implementation here is independent and contains no imported code from that project.

`api/releases.ts` is a Vercel Node function with a typed Web `Request`/`Response` boundary. It is independent of the GitHub workflow and publisher: it consumes public GitHub REST metadata at runtime and imports no `.github/` code. The website-side version and asset interpretation lives in `lib/release-contract.ts`, and `lib/release-selection.ts` owns response validation, selection, and HTTP outcomes. Neither shell installer imports TypeScript. The static Astro development/preview server does not serve this root Vercel function; exercise it with the HTTP tests or Vercel's local runtime.

`GET /api/releases` returns JSON with `version`, `preview`, release-note/checksum/provenance URLs, and a list of `{target, url}` archives. `format=text` returns just the selected version followed by a newline, for the installers. `includePreviews=true` includes numbered `dev` versions alongside full releases; omission or `false` selects full releases only. Unknown, repeated, or invalid parameters return 400. No eligible release returns 404 with guidance. Invalid upstream responses, missing selected-release assets, pagination-limit exhaustion, and upstream failures return 503; they never silently choose an older release. Successful responses are CDN-cacheable for five minutes, so a newly published version can take that long to become visible. Errors are not cached. No GitHub token or new deployment secret is required; upstream rate limiting produces a visible retryable failure. An explicit installer version bypasses this endpoint.

Functional tests run the actual scripts against local HTTP servers in disposable homes/program directories. The fixture API uses the real TypeScript selector and a separate HTTP route for synthetic GitHub metadata. Fixture archives contain an executable version reporter: a POSIX script on Linux/macOS and a compiled .NET executable on Windows. No real recipients, integration services, application databases, or production downloads are used. Private script copies replace only the fixed production origins and HTTPS transport allowlist with loopback HTTP; the shipped scripts expose no insecure-origin override. Tests exercise archive downloads, checksums, malformed contents, executable version checks, updates, downgrade rejection/override, process locks, recovery, and removal. Linux tests also drive the actual confirmation prompt through a pseudo-terminal and verify shell-profile ownership. Windows tests use `-NoModifyPath` to avoid modifying the developer's real user registry.

Run installer tests natively on Linux/macOS and Windows. The opposite platform's suite is explicitly skipped. Windows defaults to `powershell.exe` (5.1); set the test-only `RUPHISH_TEST_POWERSHELL` environment variable to the PowerShell 7 executable to run that suite again. On Linux, `script` from util-linux is required for the terminal test. The build artifact test verifies emitted script bytes, not a deployed HTTP endpoint. These tests do not establish compatibility of a published executable, macOS signing/notarization behavior, or an untested OS/architecture. Deployment checks, real release-archive smoke tests, and native platform acceptance remain separate from fixture results. The GitHub workflow files are unchanged; these new tests are invoked by the direct commands above.
