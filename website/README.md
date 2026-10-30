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
npx astro check
npx astro build
```

The Vercel configuration rebuilds for changes under `website/` or `docs/`; root `assets/` changes do not rebuild or update the website until the image is copied manually. Production canonical URLs and the sitemap use `SITE_URL` when set, otherwise `VERCEL_PROJECT_PRODUCTION_URL`.
