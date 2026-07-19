import { createRouter, RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { hydrateSettings } from './features/settings/settings-store'
import { routeTree } from './routeTree.gen'
import './index.css'

const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

const rootElement = document.getElementById('root')
if (!rootElement) throw new Error('Root element #root not found')

// Hydrate persisted settings before the first render: the cover form is
// read-once-per-mount and pool sizing reads the store synchronously. A failed
// idb read must never block boot — defaults are already in the store.
void hydrateSettings()
  .catch(() => {})
  .then(() => {
    createRoot(rootElement).render(
      <StrictMode>
        <RouterProvider router={router} />
      </StrictMode>,
    )
  })
