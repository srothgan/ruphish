// Local static preview of a completed demo build; no API or writable resources.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, relative, extname, isAbsolute } from 'node:path';
import { pathToFileURL } from 'node:url';

const types = {
  '.html': 'text/html;charset=utf-8',
  '.js': 'text/javascript;charset=utf-8',
  '.mjs': 'text/javascript;charset=utf-8',
  '.css': 'text/css;charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.woff2': 'font/woff2',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
};

export function previewServer(directory) {
  const root = resolve(directory);
  return createServer(async (request, response) => {
    response.setHeader('Cache-Control', 'no-store');
    response.setHeader('X-Robots-Tag', 'noindex, nofollow');
    if (!['GET', 'HEAD'].includes(request.method)) {
      response.writeHead(405).end();
      return;
    }
    let path;
    try {
      path = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    } catch {
      response.writeHead(400).end();
      return;
    }
    if (/[\\\0]/.test(path) || /^\/(api|auth|internal)(\/|$)/.test(path)) {
      response.writeHead(404).end();
      return;
    }
    const file = resolve(root, `.${path}`);
    const within = relative(root, file);
    if (within.startsWith('..') || isAbsolute(within)) {
      response.writeHead(404).end();
      return;
    }
    let content;
    let extension = extname(file);
    try {
      content = await readFile(file);
    } catch {
      if (extension) {
        response.writeHead(404).end();
        return;
      }
      try {
        content = await readFile(resolve(root, 'index.html'));
        extension = '.html';
      } catch {
        response.writeHead(503).end('Build the demo first.');
        return;
      }
    }
    response.setHeader('Content-Type', types[extension] ?? 'application/octet-stream');
    response.writeHead(200).end(request.method === 'HEAD' ? undefined : content);
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const server = previewServer('dist/admin');
  server.listen(8080, '127.0.0.1', () =>
    console.log('Demo preview: http://127.0.0.1:8080 (demo / demodemo)'),
  );
}
