import { defineMiddleware } from 'astro:middleware';
import { parseHTML } from 'linkedom';

// Static pages are rendered through middleware at build time. Apply the same
// native link behavior to Astro templates, Markdown and Starlight components.
export const onRequest = defineMiddleware(async ({ url }, next) => {
  const response = await next();
  if (!response.headers.get('content-type')?.includes('text/html')) return response;

  const { document } = parseHTML(await response.text());
  for (const link of document.querySelectorAll('a[href]')) {
    const href = link.getAttribute('href');
    if (!href || !URL.canParse(href, url)) continue;
    const destination = new URL(href, url);
    if (!['http:', 'https:'].includes(destination.protocol) || destination.origin === url.origin) continue;
    link.setAttribute('target', '_blank');
    const rel = new Set((link.getAttribute('rel') ?? '').split(/\s+/).filter(Boolean));
    rel.delete('opener');
    rel.add('noopener');
    link.setAttribute('rel', [...rel].join(' '));
  }

  const headers = new Headers(response.headers);
  headers.delete('content-length');
  return new Response(document.toString(), {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
});
