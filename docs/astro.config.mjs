// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import remarkGfm from 'remark-gfm';

// Project page at https://ogabrielluiz.github.io/s-bank
export default defineConfig({
  site: 'https://ogabrielluiz.github.io',
  base: '/s-bank',
  // GFM tables/strikethrough don't reach the MDX pipeline by default in this
  // Astro 6 + @astrojs/mdx combo, so wire remark-gfm in explicitly. @astrojs/mdx
  // inherits markdown.remarkPlugins (extendMarkdownConfig defaults to true).
  markdown: {
    remarkPlugins: [remarkGfm],
  },
  integrations: [
    starlight({
      title: 'S-Bank',
      logo: {
        light: './src/assets/wordmark-light.svg',
        dark: './src/assets/wordmark-dark.svg',
        replacesTitle: true,
      },
      favicon: '/favicon.svg',
      customCss: ['./src/styles/brand.css'],
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/ogabrielluiz/s-bank' },
      ],
      sidebar: [
        {
          label: 'Get started',
          items: [
            { label: 'Introduction', link: '/' },
            { label: 'Use the library', slug: 'library' },
            { label: 'Installation', slug: 'installation' },
          ],
        },
        {
          label: 'Modules',
          items: [
            { label: 'Strike', slug: 'modules/strike' },
            { label: 'Vactrol LPG', slug: 'modules/vactrol-lpg' },
          ],
        },
        {
          label: 'Develop',
          items: [{ label: 'Develop', slug: 'development' }],
        },
      ],
    }),
  ],
});
