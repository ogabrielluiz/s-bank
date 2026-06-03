// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// Project page at https://ogabrielluiz.github.io/s-bank
export default defineConfig({
  site: 'https://ogabrielluiz.github.io',
  base: '/s-bank',
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
