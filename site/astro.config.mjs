import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { site, base } from './config.mjs';

// The sidebar never lists rules: Starlight renders it into every page, so it would grow with
// rules squared. The Rules index (search and filters) is the way in.

export default defineConfig({
  site,
  base,
  outDir: process.env.SITE_OUT ?? './dist',
  integrations: [
    starlight({
      title: 'Sudsy',
      customCss: ['./src/styles/site.css'],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/a-coox/sudsy' }],
      sidebar: [
        { label: 'Getting started', link: '/getting-started/' },
        { label: 'Rules', link: '/rules/' },
        {
          label: 'Reference',
          items: [
            { label: 'Presets', link: '/reference/presets/' },
            { label: 'Format options', link: '/reference/format-options/' },
          ],
        },
      ],
    }),
  ],
});
