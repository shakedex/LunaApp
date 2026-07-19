import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'

export default defineConfig({
  site: 'https://luna.ozer2.one',
  base: '/docs',
  // Astro's `base` prefixes URLs only — it does NOT nest the emitted files.
  // The static-assets Worker matches URL paths literally against the asset
  // directory, so the tree must mirror the /docs path (Cloudflare's documented
  // "serving a subdirectory" pattern). outDir does the nesting.
  outDir: './dist/docs',
  integrations: [
    starlight({
      title: 'Luna Web',
      description: 'Camera reports in your browser. Nothing leaves your device.',
      social: [
        { 
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/shakedex/LunaApp'
        },
      ],
    }),
  ],
})
