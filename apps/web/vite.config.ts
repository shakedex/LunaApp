import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'
import { cloudflare } from '@cloudflare/vite-plugin'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig, lazyPlugins } from 'vite-plus'

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as {
  version: string
}

// The desktop shell embeds this build into a binary, so it must not contain
// Cloudflare Worker artifacts (wrangler.json carries the build machine's absolute path).
const isDesktop = process.env.LUNA_TARGET === 'desktop'

export default defineConfig({
  plugins: lazyPlugins(() => [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    tailwindcss(),
    ...(isDesktop ? [] : [cloudflare()]),
  ]),
  build: { outDir: isDesktop ? 'dist-desktop' : 'dist' },
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  test: {
    include: ['src/**/*.test.ts'],
  },
})
