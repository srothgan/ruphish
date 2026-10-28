import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { visit } from 'unist-util-visit';

export const sourceRoot = resolve(fileURLToPath(new URL('../../docs/', import.meta.url)));
export const contentRoot = fileURLToPath(new URL('../src/content/docs/', import.meta.url));
export const generatedRoot = resolve(contentRoot, 'docs');
const assetsRoot = fileURLToPath(new URL('../public/docs-files/', import.meta.url));
const repositoryRoot = resolve(sourceRoot, '..');
const repository = 'https://github.com/srothgan/ruphish';
const slash = (value) => value.split(sep).join('/');
export const isPublished = (path) =>
  !slash(path)
    .split('/')
    .some((part) => part === 'old' || part.startsWith('.') || part.startsWith('_'));

export function filesBelow(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(root, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Documentation must not contain symlinks: ${path}`);
    }
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

export function docRoute(path) {
  const stem = slash(path)
    .replace(/\.(md|mdx)$/, '')
    .replace(/(^|\/)index$/, '');
  return `/docs/${stem}`.replace(/\/+$/, '') + '/';
}

/** Disposable Starlight input. Authors only edit ../docs; no committed mirrors. */
export function syncDocs() {
  const expected = new Set();
  const routes = new Set();
  for (const source of filesBelow(sourceRoot)) {
    const name = relative(sourceRoot, source);
    if (!isPublished(name)) {
      continue;
    }
    const markdown = /\.mdx?$/.test(source);
    const destination = resolve(markdown ? generatedRoot : assetsRoot, name);
    expected.add(destination);
    mkdirSync(dirname(destination), { recursive: true });
    if (markdown) {
      const route = docRoute(name);
      if (routes.has(route)) {
        throw new Error(`Duplicate documentation route: ${route}`);
      }
      routes.add(route);
    }
    const bytes = readFileSync(source);
    if (!existsSync(destination) || !bytes.equals(readFileSync(destination))) {
      writeFileSync(destination, bytes);
    }
  }
  for (const generated of [generatedRoot, assetsRoot]) {
    if (!existsSync(generated)) {
      continue;
    }
    for (const obsolete of filesBelow(generated)) {
      if (!expected.has(obsolete)) {
        rmSync(obsolete);
      }
    }
  }
}

/** Resolve source-oriented Markdown links without touching code examples or prose. */
export function resolveDocLink(url, source) {
  if (/^(?:[a-z][a-z\d+.-]*:|\/|#)/i.test(url)) {
    return url;
  }
  const match = url.match(/^([^?#]*)(.*)$/);
  const target = resolve(dirname(source), decodeURIComponent(match[1]));
  const suffix = match[2];
  const within = relative(sourceRoot, target);
  if (within.startsWith('..') || within.startsWith(sep)) {
    const repoPath = relative(repositoryRoot, target);
    if (repoPath.startsWith('..') || !existsSync(target)) {
      throw new Error(`Missing source link: ${url} in ${source}`);
    }
    return `${repository}/blob/main/${slash(repoPath).split('/').map(encodeURIComponent).join('/')}${suffix}`;
  }
  if (!isPublished(within) || !existsSync(target)) {
    throw new Error(`Missing or unpublished documentation link: ${url} in ${source}`);
  }
  if (/\.mdx?$/.test(target)) {
    return docRoute(within) + suffix;
  }
  return `/docs-files/${slash(within).split('/').map(encodeURIComponent).join('/')}${suffix}`;
}

export function remarkDocLinks() {
  return (tree, file) => {
    const name = relative(generatedRoot, file.path);
    if (name.startsWith('..')) {
      return;
    }
    const source = resolve(sourceRoot, name);
    visit(tree, (node) => {
      if (node.type === 'link' || node.type === 'definition' || node.type === 'image') {
        node.url = resolveDocLink(node.url, source);
      }
    });
  };
}

export function documentationIntegration() {
  let timer;
  return {
    name: 'ruphish-documentation',
    hooks: {
      'astro:config:setup': () => syncDocs(),
      'astro:server:setup': ({ server }) => {
        server.watcher.add([sourceRoot]);
        const changed = (path) => {
          if (!resolve(path).startsWith(sourceRoot + sep)) {
            return;
          }
          clearTimeout(timer);
          timer = setTimeout(() => {
            try {
              syncDocs();
            } catch (error) {
              server.ws.send({
                type: 'error',
                err: { message: error.message, stack: error.stack },
              });
            }
          }, 80);
        };
        server.watcher.on('add', changed).on('change', changed).on('unlink', changed);
        server.httpServer?.once('close', () => clearTimeout(timer));
      },
    },
  };
}
