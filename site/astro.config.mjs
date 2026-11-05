import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { site, base, rules } from './config.mjs';

// The sidebar never lists rules: Starlight renders it into every page, so it would grow with
// rules squared. The index page's filters (reachable by language below) are the way in.
const languages = [...new Set(rules().flatMap((r) => r.languages))].sort();

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
        {
          label: 'Rules',
          collapsed: true,
          items: [
            { label: 'Search all rules', link: '/rules/' },
            ...languages.map((l) => ({ label: l, link: `/rules/?language=${l}` })),
          ],
        },
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
