import starlight from '@astrojs/starlight'
import { defineConfig } from 'astro/config'

export default defineConfig({
  site: 'https://luna.ozer2.one',
  base: '/docs',
  integrations: [
    starlight({
      title: 'Luna Web',
      description: 'Camera reports in your browser. Nothing leaves your device.',
      social: [],
    }),
  ],
})
