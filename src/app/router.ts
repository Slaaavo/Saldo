import { createRootRoute, createRoute, createRouter, createMemoryHistory, redirect } from '@tanstack/react-router'
import App from './App'
import DashboardView from '../features/dashboard/DashboardView'
import LedgerPage from '../features/ledger/LedgerPage'
import FxRatesPage from '../features/currency/FxRatesPage'
import UnitsPage from '../features/assets/UnitsPage'
import SettingsPage from '../features/settings/SettingsPage'
import PartnersPage from '../features/partners/PartnersPage'
import ImportProfilesPage from '../features/csv-profiles/ImportProfilesPage'
import PersonsPage from '../features/persons/PersonsPage'

// Root route — App is the shell layout (renders Outlet inside)
const rootRoute = createRootRoute({
  component: App,
})

// Index route: redirect / → /dashboard
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: '/dashboard' })
  },
})

// Page routes — components are now self-fetching with no required props
const dashboardRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/dashboard',
  component: DashboardView,
})

const ledgerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/ledger',
  component: LedgerPage,
})

const fxRatesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/fx-rates',
  component: FxRatesPage,
})

const unitsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/units',
  component: UnitsPage,
})

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/settings',
  component: SettingsPage,
})

const partnersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/partners',
  component: PartnersPage,
})

const importProfilesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/import-profiles',
  component: ImportProfilesPage,
})

const personsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/persons',
  component: PersonsPage,
})

export const routeTree = rootRoute.addChildren([indexRoute, dashboardRoute, ledgerRoute, fxRatesRoute, unitsRoute, settingsRoute, partnersRoute, importProfilesRoute, personsRoute])

const memoryHistory = createMemoryHistory({ initialEntries: ['/dashboard'] })

export const router = createRouter({ routeTree, history: memoryHistory })

// Register the router type for full TypeScript inference on useNavigate, Link, etc.
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

export default router
