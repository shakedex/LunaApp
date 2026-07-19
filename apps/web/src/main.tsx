import { createRouter, RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { hydrateSettings } from './features/settings/settings-store'
import { hydrateActivity } from './lib/logger'
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

// Hydrate persisted settings and activity before the first render: the cover
// form is read-once-per-mount, pool sizing reads the store synchronously, and
// the activity log's ids must not collide with restored ones. A failed idb
// read must never block boot — defaults are already in the stores.
void Promise.allSettled([hydrateSettings(), hydrateActivity()]).then(() => {
  createRoot(rootElement).render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  )
})
