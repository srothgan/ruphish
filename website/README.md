# Ruphish website

The standalone Astro website contains the landing page, About page, and Starlight documentation. Run these commands from `ruphish/website/` with Node.js 26.8.1.

## Local development

Install dependencies on a fresh checkout:

```sh
npm ci
```

Start the development server:

```sh
npx astro dev --host 127.0.0.1 --port 4321
```

Open <http://127.0.0.1:4321/>. Astro reloads website edits and synchronizes changes to the canonical Markdown under `../docs/`. The generated `src/content/docs/`, `public/docs-files/`, and `public/assets/` directories are ignored build inputs; edit the original sources.

## Preview with documentation search

Starlight generates its Pagefind search index during the build. Build and preview to use the complete search experience locally:

```sh
npx astro build
node tools/check-site.mjs
npx astro preview --host 127.0.0.1 --port 4321
```

Stop the running server with Ctrl+C before starting another server on the same port.

## Website assets

The R-only favicon is `public/favicon.svg`. The full wordmark, its light variant, the R icons, and the landing-page dashboard screenshot are in `src/assets/`. The documentation uses the shared screenshot at `../assets/dashboard.png`; refresh both screenshot copies together. Synthetic data, the Docker demo, and screenshot capture tooling stay outside this repository.

## Validation

```sh
node --test tools/docs.test.mjs tools/ignore-build.test.mjs
npx astro check
npx astro build
node tools/check-site.mjs
```

The website CI job validates changes under `website/`, `docs/`, and `assets/`. The Vercel configuration uses the same paths to decide whether a deployment needs rebuilding. Production canonical URLs and the sitemap use `SITE_URL` when set, otherwise `VERCEL_PROJECT_PRODUCTION_URL`.
