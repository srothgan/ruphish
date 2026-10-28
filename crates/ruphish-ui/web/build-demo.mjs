// Generate the demo browser bundle and HTML entry point before a demo build or standalone WASM demo check.
import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { readFile, writeFile } from 'node:fs/promises';
import { starterContent, starterNotices } from '../../../assets/website-demo/starter-content.mjs';
await build({
    entryPoints: [fileURLToPath(new URL('./demo-http.mjs', import.meta.url))],
    outfile: fileURLToPath(new URL('./generated/demo.js', import.meta.url)),
    bundle: true, format: 'esm', target: 'es2022', minify: true, legalComments: 'linked',
    banner: { js: `/*! Bundled starter content notices\n${starterNotices.replaceAll('*/', '* /')}\n*/` },
    plugins: [{ name: 'static-starter-content', setup(build) {
        build.onLoad({ filter: /[\\/]assets[\\/]website-demo[\\/]starter-content\.mjs$/ }, () => ({ contents: `export const starterContent = ${JSON.stringify(starterContent)};`, loader: 'js' }));
    } }],
});

// Share the application entry point and assets, with a static demo-specific SEO title.
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8');
if (!html.includes('<title>Ruphish</title>')) throw new Error('Missing application title');
await writeFile(new URL('./generated/index.html', import.meta.url), html
    .replace('<title>Ruphish</title>', '<title>Ruphish Demo</title>')
    .replace(/href="(?!\/|[a-z]+:)([^"]+)"/g, (_, href) => `href="../../${href}"`));
