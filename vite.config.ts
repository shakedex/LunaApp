import { defineConfig } from 'vite-plus'

// Paths Biome excluded — must stay excluded from lint and format:
// docs app (checked by astro check), local-only root docs/, styles/assets,
// vendored shadcn components, generated router tree, throwaway analysis tools.
const excluded = [
  'apps/docs/**',
  'docs/**',
  '**/*.css',
  '**/*.svg',
  'apps/web/src/components/ui/**',
  '**/routeTree.gen.ts',
  'tools/analysis/**',
  '**/dist/**',
]

export default defineConfig({
  fmt: {
    singleQuote: true,
    semi: false,
    printWidth: 100,
    ignorePatterns: excluded,
  },
  lint: {
    // Known regression vs Biome: no import sorting in vite-plus 0.2.x
    // (organizeImports equivalent absent from oxfmt/oxlint) — revisit at 0.3.
    ignorePatterns: excluded,
    plugins: ['typescript'],
    jsPlugins: [{ name: 'vite-plus', specifier: 'vite-plus/oxlint-plugin' }],
    rules: { 'vite-plus/prefer-vite-plus-imports': 'error' },
    options: { typeAware: true, typeCheck: true },
    overrides: [
      {
        files: ['apps/web/**'],
        plugins: ['typescript', 'react'],
      },
      {
        files: ['**/*.test.ts'],
        plugins: ['typescript', 'vitest'],
        rules: {
          // Test fixtures spread 4-char ASCII box codes ([...'moov']) into byte
          // arrays — code-point splitting is exactly what's wanted there.
          'typescript/no-misused-spread': 'off',
        },
      },
    ],
  },
})
