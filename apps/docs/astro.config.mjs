import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'
import starlightThemeBlack from 'starlight-theme-black'

export default defineConfig({
  site: 'https://luna.ozer2.one',
  base: '/docs',
  trailingSlash: 'always',
  // Astro's `base` prefixes URLs only — it does NOT nest the emitted files.
  // The static-assets Worker matches URL paths literally against the asset
  // directory, so the tree must mirror the /docs path (Cloudflare's documented
  // "serving a subdirectory" pattern). outDir does the nesting.
  outDir: './dist/docs',
  integrations: [
    starlight({
      title: 'Luna',
      description: 'Camera reports in your browser. Nothing leaves your device.',
      // Tab icon + link/share icon (the Luna cat+moon mark).
      favicon: '/favicon.ico',
      head: [
        {
          tag: 'link',
          attrs: { rel: 'apple-touch-icon', href: '/docs/apple-touch-icon.png' },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image', content: 'https://luna.ozer2.one/docs/og.png' },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image:width', content: '1200' },
        },
        {
          tag: 'meta',
          attrs: { property: 'og:image:height', content: '630' },
        },
        {
          tag: 'meta',
          attrs: { name: 'twitter:image', content: 'https://luna.ozer2.one/docs/og.png' },
        },
      ],
      plugins: [
        starlightThemeBlack({
          // Off-brand clutter for a product docs site — no AI-tool buttons.
          docs: { showMarkdownActions: false },
          navLinks: [{ label: 'Open Luna', link: 'https://luna.ozer2.one/' }],
        }),
      ],
      customCss: ['./src/styles/cinema-dark.css'],
      // Dark-only site: drop the theme toggle; add a Luna site footer.
      components: {
        ThemeSelect: './src/components/ThemeSelect.astro',
        Footer: './src/components/Footer.astro',
      },
      logo: {
        src: './src/assets/luna-logo-lg.webp',
      },
      social: [
        { 
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/shakedex/LunaApp'
        },
      ],
      sidebar: [
        { label: 'Overview', link: '/' },
        { label: 'Privacy', slug: 'privacy' },
        { label: 'Supported formats', slug: 'supported-formats' },
        { label: 'Limitations', slug: 'limitations' },
        { label: 'FAQ', slug: 'faq' },
        { label: 'Changelog', slug: 'changelog' },
      ],
    }),
  ],
})
