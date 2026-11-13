import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { createMemoryHistory, createRootRoute, createRoute, createRouter, RouterProvider } from '@tanstack/react-router'
import Sidebar from './Sidebar'

vi.mock('react-i18next', async () => (await import('../testing/i18nMock')).i18nMock)

const SidebarShell = () => <Sidebar collapsed={false} onToggleCollapse={() => {}} />

const renderSidebarAt = (entry: string) => {
  const rootRoute = createRootRoute({ component: SidebarShell })
  const routeTree = rootRoute.addChildren([createRoute({ getParentRoute: () => rootRoute, path: '/dashboard' }), createRoute({ getParentRoute: () => rootRoute, path: '/ledger' })])
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [entry] }) })
  render(<RouterProvider router={router} />)
}

describe('Sidebar', () => {
  it('marks the page the user is on', async () => {
    renderSidebarAt('/ledger')

    expect(await screen.findByRole('link', { name: 'sidebar.ledger' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('link', { name: 'sidebar.dashboard' })).not.toHaveAttribute('aria-current')
  })

  it('still marks the ledger when it was opened filtered to an account', async () => {
    renderSidebarAt('/ledger?accountId=5')

    expect(await screen.findByRole('link', { name: 'sidebar.ledger' })).toHaveAttribute('aria-current', 'page')
  })

  it('lists Settings last, after the money pages and the setup lists', async () => {
    renderSidebarAt('/dashboard')

    const labels = (await screen.findAllByRole('link')).map((link) => link.textContent)

    expect(labels).toEqual([
      'sidebar.dashboard',
      'sidebar.ledger',
      'sidebar.taxModels',
      'sidebar.persons',
      'sidebar.partners',
      'sidebar.importProfiles',
      'sidebar.fxRates',
      'sidebar.units',
      'sidebar.settings',
    ])
  })
})
