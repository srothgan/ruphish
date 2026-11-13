import type { APIRoute } from 'astro';

// Allow full crawling and point crawlers at the generated sitemap index.
// Use the canonical domain configured in astro.config.mjs.
export const GET: APIRoute = ({ site }) => {
  const lines = ['User-agent: *', 'Allow: /'];
  if (site) lines.push('', `Sitemap: ${new URL('sitemap-index.xml', site).href}`);
  return new Response(`${lines.join('\n')}\n`, {
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
  });
};
