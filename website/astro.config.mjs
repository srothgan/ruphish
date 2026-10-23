import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { unified } from '@astrojs/markdown-remark';
import { existsSync } from 'node:fs';
import { documentationIntegration, remarkDocLinks, sourceRoot } from './tools/docs.mjs';
import { resolve } from 'node:path';

const sections = [
  ['Getting started', 'getting-started'],
  ['User guide', 'user-guide'],
  ['Administration', 'administration'],
  ['HTTP API', 'api'],
  ['Migration from Gophish', 'migration'],
  ['Contributing', 'contributing'],
];

// Production uses its assigned Vercel domain; a custom domain can override SITE_URL.
const site = process.env.SITE_URL || (process.env.VERCEL_PROJECT_PRODUCTION_URL
  ? `https://${process.env.VERCEL_PROJECT_PRODUCTION_URL}` : undefined);

export default defineConfig({
  site,
  output: 'static',
  trailingSlash: 'always',
  integrations: [
    documentationIntegration(),
    starlight({
      title: 'Ruphish',
      description: 'Guides and reference for running phishing simulations with Ruphish.',
      logo: { src: './src/assets/ruphish-wordmark-white.svg', alt: 'Ruphish', replacesTitle: true },
      favicon: '/favicon.svg',
      social: [{ icon: 'github', label: 'Ruphish on GitHub', href: 'https://github.com/srothgan/ruphish' }],
      customCss: ['./src/styles/fonts.css', './src/styles/docs.css'],
      components: { Head: './src/components/DocsHead.astro', SocialIcons: './src/components/DocsLinks.astro', Footer: './src/components/DocsFooter.astro' },
      routeMiddleware: './src/docs-middleware.ts',
      sidebar: [
        { label: 'Documentation', slug: 'docs' },
        ...sections.filter(([, directory]) => existsSync(resolve(sourceRoot, directory))).map(([label, directory]) => ({
          label, items: [{ autogenerate: { directory: `docs/${directory}` } }], collapsed: directory !== 'getting-started',
        })),
      ],
      tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
      pagination: true,
      credits: true,
    }),
  ],
  markdown: { processor: unified({ remarkPlugins: [remarkDocLinks] }) },
});
