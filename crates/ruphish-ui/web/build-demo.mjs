// Bundle only the demo's browser modules. Run before a demo build or standalone WASM demo check.
import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
await build({
    entryPoints: [fileURLToPath(new URL('./demo-http.mjs', import.meta.url))],
    outfile: fileURLToPath(new URL('./generated/demo.js', import.meta.url)),
    bundle: true, format: 'esm', target: 'es2022', minify: true, legalComments: 'linked',
});
