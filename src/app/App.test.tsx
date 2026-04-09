import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createRouter, createRootRoute, createRoute, createMemoryHistory, RouterProvider } from '@tanstack/react-router'
import type { RouteComponent } from '@tanstack/react-router'
import App from './App'
import DashboardView from '../features/dashboard/DashboardView'
import SettingsPage from '../features/settings/SettingsPage'
import FxRatesPage from '../features/currency/FxRatesPage'
import UnitsPage from '../features/assets/UnitsPage'
import LedgerPage from '../features/ledger/LedgerPage'
import PartnersPage from '../features/partners/PartnersPage'
import ImportProfilesPage from '../features/csv-profiles/ImportProfilesPage'
import type { SnapshotRow, EventWithData, Currency, ModalState, DbLocationInfo } from '../shared/types'
import type { ThemePreference } from '../features/settings/useTheme'

// ── Mock i18n — pass-through that returns the key (with interpolations) ─────
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v))
        }
        return result
      }
      return key
    },
    i18n: { language: 'en' },
  }),
}))

// ── Mock sonner ─────────────────────────────────────────────────────────────
const mockToastSuccess = vi.fn()
const mockToastError = vi.fn()
const mockToastWarning = vi.fn()

vi.mock('sonner', () => ({
  Toaster: () => <div data-testid="toaster" />,
  toast: {
    success: (...args: unknown[]) => mockToastSuccess(...args),
    error: (...args: unknown[]) => mockToastError(...args),
    warning: (...args: unknown[]) => mockToastWarning(...args),
  },
}))

// ── Mock API ────────────────────────────────────────────────────────────────
const mockFetchFxRates = vi.fn()
const mockEnterDemoMode = vi.fn()
const mockExitDemoMode = vi.fn()
const mockIsDemoMode = vi.fn()
const mockGetDbLocation = vi.fn()
const mockPickDbFolder = vi.fn()
const mockChangeDbLocation = vi.fn()
const mockResetDbLocation = vi.fn()
const mockCheckDefaultDb = vi.fn()
const mockGetBulkUpdateExclusions = vi.fn()
const mockGetAccountsSnapshot = vi.fn()
const mockGetConsolidationCurrency = vi.fn()
const mockListEvents = vi.fn()

vi.mock('../shared/api', () => ({
  fetchFxRates: (...args: unknown[]) => mockFetchFxRates(...args),
  enterDemoMode: (...args: unknown[]) => mockEnterDemoMode(...args),
  exitDemoMode: (...args: unknown[]) => mockExitDemoMode(...args),
  isDemoMode: (...args: unknown[]) => mockIsDemoMode(...args),
  getDbLocation: (...args: unknown[]) => mockGetDbLocation(...args),
  pickDbFolder: (...args: unknown[]) => mockPickDbFolder(...args),
  changeDbLocation: (...args: unknown[]) => mockChangeDbLocation(...args),
  resetDbLocation: (...args: unknown[]) => mockResetDbLocation(...args),
  checkDefaultDb: (...args: unknown[]) => mockCheckDefaultDb(...args),
  createBalanceUpdate: vi.fn(),
  getAccountsSnapshot: (...args: unknown[]) => mockGetAccountsSnapshot(...args),
  listEvents: (...args: unknown[]) => mockListEvents(...args),
  createAccount: vi.fn(),
  updateAccount: vi.fn(),
  createPartnerAccount: vi.fn(),
  listPartnerAccounts: vi.fn(),
  updatePartnerAccount: vi.fn(),
  deletePartnerAccount: vi.fn(),
  deleteAccount: vi.fn(),
  updateEvent: vi.fn(),
  deleteEvent: vi.fn(),
  bulkCreateBalanceUpdates: vi.fn(),
  listCurrencies: vi.fn(),
  getConsolidationCurrency: (...args: unknown[]) => mockGetConsolidationCurrency(...args),
  setConsolidationCurrency: vi.fn(),
  setFxRateManual: vi.fn(),
  listFxRates: vi.fn(),
  getMissingRateDates: vi.fn(),
  getAppSetting: vi.fn(),
  createBucketBalanceUpdate: vi.fn(),
  updateBucketBalanceUpdate: vi.fn(),
  listLinksForEvent: vi.fn().mockResolvedValue([]),
  setAppSetting: vi.fn(),
  updateSortOrder: vi.fn(),
  createCustomUnit: vi.fn(),
  listCustomUnits: vi.fn(),
  updateCustomUnit: vi.fn(),
  updateAssetValue: vi.fn(),
  listAccountAssetLinks: vi.fn(),
  setAccountAssetLinks: vi.fn(),
  getBulkUpdateExclusions: (...args: unknown[]) => mockGetBulkUpdateExclusions(...args),
  setBulkUpdateExclusions: vi.fn(),
}))

// ── Mock hooks ──────────────────────────────────────────────────────────────
const mockSetThemePreference = vi.fn()
let themeReturn = {
  theme: 'light' as 'light' | 'dark',
  themePreference: 'system' as ThemePreference,
  setThemePreference: mockSetThemePreference,
}

vi.mock('../features/settings/useTheme', () => ({
  useTheme: () => themeReturn,
}))

const mockSetModalState = vi.fn()
const mockCloseModal = vi.fn()
let modalStateValue: ModalState = { type: 'none' }

vi.mock('./useModalManager', () => ({
  useModalManager: () => ({
    modalState: modalStateValue,
    setModalState: mockSetModalState,
    closeModal: mockCloseModal,
  }),
}))

const mockHandleCreateBalanceUpdate = vi.fn()
const mockHandleEditBalanceUpdate = vi.fn()
const mockHandleDeleteEvent = vi.fn()
const mockHandleCreateAccount = vi.fn()
const mockHandleEditAccount = vi.fn()
const mockHandleDeleteAccount = vi.fn()
const mockHandleBulkUpdateSubmit = vi.fn()
const mockHandleSaveOrder = vi.fn()
const mockHandleUpdateAssetValue = vi.fn()
const mockHandleSetAccountAssetLinks = vi.fn()

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

vi.mock('./useModalActions', () => ({
  useModalActions: () => ({
    handleCreateBalanceUpdate: mockHandleCreateBalanceUpdate,
    handleEditBalanceUpdate: mockHandleEditBalanceUpdate,
    handleDeleteEvent: mockHandleDeleteEvent,
    handleCreateAccount: mockHandleCreateAccount,
    handleEditAccount: mockHandleEditAccount,
    handleDeleteAccount: mockHandleDeleteAccount,
    handleBulkUpdateSubmit: mockHandleBulkUpdateSubmit,
    handleSaveOrder: mockHandleSaveOrder,
    handleUpdateAssetValue: mockHandleUpdateAssetValue,
    handleSetAccountAssetLinks: mockHandleSetAccountAssetLinks,
    handleCreateAssetSuccess: vi.fn().mockImplementation(async () => {
      mockCloseModal()
    }),
  }),
}))

// ── Mock child components ───────────────────────────────────────────────────
vi.mock('../shared/layout/Header', async () => {
  const { useRouterState } = await import('@tanstack/react-router')

  const HeaderMock = ({ pageTitle }: { pageTitle: string }) => {
    const pathname = useRouterState({ select: (s: { location: { pathname: string } }) => s.location.pathname })
    const showDatePicker = pathname === '/dashboard'
    return (
      <div data-testid="header">
        <span data-testid="page-title">{pageTitle}</span>
        {showDatePicker && <span data-testid="date-picker-visible" />}
      </div>
    )
  }

  return { default: HeaderMock }
})

vi.mock('../shared/layout/Sidebar', async () => {
  const { useNavigate, useRouterState } = await import('@tanstack/react-router')

  const SidebarMock = ({ collapsed, onToggleCollapse }: { collapsed: boolean; onToggleCollapse: () => void }) => {
    const navigate = useNavigate()
    const pathname = useRouterState({ select: (s: { location: { pathname: string } }) => s.location.pathname })
    return (
      <div data-testid="sidebar">
        <span data-testid="current-view">{pathname.slice(1) || 'dashboard'}</span>
        <button data-testid="nav-dashboard" onClick={() => navigate({ to: '/dashboard' })}>
          Dashboard
        </button>
        <button data-testid="nav-settings" onClick={() => navigate({ to: '/settings' })}>
          Settings
        </button>
        <button data-testid="nav-fx-rates" onClick={() => navigate({ to: '/fx-rates' })}>
          FxRates
        </button>
        <button data-testid="nav-units" onClick={() => navigate({ to: '/units' })}>
          Units
        </button>
        <button data-testid="toggle-collapse" onClick={onToggleCollapse}>
          Toggle
        </button>
        <span data-testid="sidebar-collapsed">{String(collapsed)}</span>
      </div>
    )
  }

  return { default: SidebarMock }
})

vi.mock('../features/settings/DemoModeBanner', () => ({
  default: (props: { onExit: () => void }) => (
    <div data-testid="demo-banner">
      <button data-testid="exit-demo" onClick={props.onExit}>
        Exit Demo
      </button>
    </div>
  ),
}))

vi.mock('../features/dashboard/DashboardView', () => ({
  default: () => <div data-testid="dashboard-view" />,
}))

vi.mock('../features/settings/SettingsPage', async () => {
  const { useDemo } = await import('./useDemo')

  const SettingsPageMock = () => {
    const demo = useDemo()
    return (
      <div data-testid="settings-page">
        <button data-testid="enter-demo" onClick={demo.onEnterDemoMode}>
          Enter Demo
        </button>
        <button data-testid="exit-demo-settings" onClick={demo.onExitDemoMode}>
          Exit Demo
        </button>
        <span data-testid="demo-mode-value">{String(demo.isDemoMode)}</span>
      </div>
    )
  }

  return { default: SettingsPageMock }
})

vi.mock('../features/currency/FxRatesPage', () => ({
  default: () => <div data-testid="fx-rates-page" />,
}))

vi.mock('../features/assets/UnitsPage', () => ({
  default: () => <div data-testid="units-page" />,
}))

vi.mock('../features/ledger/LedgerPage', () => ({
  default: () => <div data-testid="ledger-page" />,
}))

vi.mock('../features/partners/PartnersPage', () => ({
  default: () => <div data-testid="partners-page" />,
}))

vi.mock('../features/csv-profiles/ImportProfilesPage', () => ({
  default: () => <div data-testid="import-profiles-page" />,
}))

// Mock all modals
vi.mock('../features/transactions/CreateBalanceUpdateModal', () => ({
  default: (props: { onSubmit: () => void; onClose: () => void }) => (
    <div data-testid="create-balance-modal">
      <button data-testid="create-balance-submit" onClick={props.onSubmit}>
        Submit
      </button>
      <button data-testid="create-balance-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/transactions/EditBalanceUpdateModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="edit-balance-modal">
      <button data-testid="edit-balance-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/accounts/CreateAccountModal', () => ({
  default: (props: { accountType?: string; onClose: () => void }) => (
    <div data-testid="create-account-modal">
      <span data-testid="create-account-type">{props.accountType}</span>
      <button data-testid="create-account-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/assets/CreateAssetModal', () => ({
  default: (props: { onSuccess: () => void; onClose: () => void }) => (
    <div data-testid="create-asset-modal">
      <button data-testid="create-asset-success" onClick={props.onSuccess}>
        Success
      </button>
      <button data-testid="create-asset-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/assets/UpdateAssetValueModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="update-asset-modal">
      <button data-testid="update-asset-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/accounts/EditAccountModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="edit-account-modal">
      <button data-testid="edit-account-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../shared/ui/ConfirmDialog', () => ({
  default: (props: { message: string; onConfirm: () => void; onCancel: () => void }) => (
    <div data-testid="confirm-dialog">
      <span data-testid="confirm-message">{props.message}</span>
      <button data-testid="confirm-ok" onClick={props.onConfirm}>
        Confirm
      </button>
      <button data-testid="confirm-cancel" onClick={props.onCancel}>
        Cancel
      </button>
    </div>
  ),
}))

vi.mock('../features/transactions/BulkUpdateBalanceModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="bulk-update-modal">
      <button data-testid="bulk-update-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/settings/DbLocationChoiceDialog', () => ({
  default: (props: { onAction: (action: string) => void; onCancel: () => void }) => (
    <div data-testid="db-choice-dialog">
      <button data-testid="db-choice-move" onClick={() => props.onAction('move')}>
        Move
      </button>
      <button data-testid="db-choice-fresh" onClick={() => props.onAction('fresh')}>
        Fresh
      </button>
      <button data-testid="db-choice-cancel" onClick={props.onCancel}>
        Cancel
      </button>
    </div>
  ),
}))

vi.mock('../shared/ui/ReorderModal', () => ({
  default: (props: { title: string; onSave: () => void; onClose: () => void }) => (
    <div data-testid="reorder-modal">
      <span data-testid="reorder-title">{props.title}</span>
      <button data-testid="reorder-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/assets/ManageLinkedAssetsModal', () => ({
  default: (props: { onClose: () => void }) => (
    <div data-testid="manage-linked-assets-modal">
      <button data-testid="manage-links-close" onClick={props.onClose}>
        Close
      </button>
    </div>
  ),
}))

vi.mock('../features/transactions/CsvImportModal', () => ({
  default: () => <div data-testid="csv-import-modal" />,
}))

vi.mock('../features/transactions/EditTransferModal', () => ({
  default: () => <div data-testid="edit-transfer-modal" />,
}))

const makeSnapshot = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  return {
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    balanceMinor: 100000,
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    isCustom: false,
    convertedBalanceMinor: 100000,
    fxRateMissing: false,
    isLinkedToAsset: false,
    linkedAssetIds: [],
    iban: null,
    isBucketLinked: false,
    bucketLinks: [],
    linkedBalanceMinor: 0,
    cashflowTaggedMinor: 0,
    personId: null,
    purchasePriceMinor: null,
    purchaseDate: null,
    depreciationPeriodMonths: null,
    ...overrides,
  }
}

const makeBucket = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  return makeSnapshot({
    accountId: 10,
    accountName: 'Vacation',
    accountType: 'bucket',
    balanceMinor: 50000,
    convertedBalanceMinor: 50000,
    ...overrides,
  })
}

const makeAsset = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  return makeSnapshot({
    accountId: 20,
    accountName: 'House',
    accountType: 'asset',
    balanceMinor: 30000000,
    convertedBalanceMinor: 30000000,
    ...overrides,
  })
}

const makeEvent = (overrides?: Partial<EventWithData>): EventWithData => {
  return {
    id: 1,
    accountId: 1,
    accountName: 'Checking',
    accountType: 'account',
    eventType: 'balance_update',
    eventDate: '2026-01-15T10:00:00',
    amountMinor: 100000,
    note: null,
    createdAt: '2026-01-15T10:00:00',
    currencyCode: 'EUR',
    currencyMinorUnits: 2,
    counterpartAccountId: null,
    counterpartAccountName: null,
    bucketId: null,
    bucketName: null,
    originalCurrencyId: null,
    originalCurrencyCode: null,
    originalAmountMinor: null,
    originalCurrencyMinorUnits: null,
    fxRateMantissa: null,
    fxRateExponent: null,
    linkedEventId: null,
    splitGroupId: null,
    splitGroupNote: null,
    vatRateBps: null,
    vatDeductiblePctBps: null,
    expenseDeductiblePctBps: null,
    prepaidPeriodMonths: null,
    isLinkedToTaxable: false,
    linkedTaxableEventId: null,
    hasLinkedCashflows: false,
    linkedCashflowCount: 0,
    linkedAssetId: null,
    isSystemGenerated: false,
    ...overrides,
  }
}

const defaultDbLocation: DbLocationInfo = {
  currentPath: '/default/path',
  isDefault: true,
  isDemoMode: false,
  fallbackWarning: false,
}

const setupDefaultMocks = (overrides?: { snapshot?: SnapshotRow[] }) => {
  mockIsDemoMode.mockResolvedValue(false)
  mockGetDbLocation.mockResolvedValue(defaultDbLocation)
  mockEnterDemoMode.mockResolvedValue(undefined)
  mockExitDemoMode.mockResolvedValue(undefined)
  mockFetchFxRates.mockResolvedValue([])
  mockPickDbFolder.mockResolvedValue(null)
  mockChangeDbLocation.mockResolvedValue(undefined)
  mockResetDbLocation.mockResolvedValue(undefined)
  mockCheckDefaultDb.mockResolvedValue(false)
  mockGetBulkUpdateExclusions.mockResolvedValue([])
  mockGetAccountsSnapshot.mockResolvedValue(overrides?.snapshot ?? [makeSnapshot()])
  mockGetConsolidationCurrency.mockResolvedValue(EUR)
  mockListEvents.mockResolvedValue({ events: [], totalCount: 0 })
}

const createTestRouter = (initialPath = '/dashboard') => {
  const rootRoute = createRootRoute({ component: App })
  const indexRoute = createRoute({ getParentRoute: () => rootRoute, path: '/' })
  const dashboardRoute = createRoute({ getParentRoute: () => rootRoute, path: '/dashboard', component: DashboardView })
  const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: SettingsPage as unknown as RouteComponent })
  const fxRatesRoute = createRoute({ getParentRoute: () => rootRoute, path: '/fx-rates', component: FxRatesPage })
  const unitsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/units', component: UnitsPage })
  const ledgerRoute = createRoute({ getParentRoute: () => rootRoute, path: '/ledger', component: LedgerPage })
  const partnersRoute = createRoute({ getParentRoute: () => rootRoute, path: '/partners', component: PartnersPage })
  const importRoute = createRoute({ getParentRoute: () => rootRoute, path: '/import-profiles', component: ImportProfilesPage })
  const routeTree = rootRoute.addChildren([indexRoute, dashboardRoute, settingsRoute, fxRatesRoute, unitsRoute, ledgerRoute, partnersRoute, importRoute])
  return createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [initialPath] }) })
}

const renderApp = (initialPath = '/dashboard') => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const router = createTestRouter(initialPath)
  return render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    modalStateValue = { type: 'none' }
    themeReturn = {
      theme: 'light',
      themePreference: 'system',
      setThemePreference: mockSetThemePreference,
    }
    setupDefaultMocks()
  })

  // ── Initial render & mount behavior ──

  describe('initial render', () => {
    it('renders the sidebar and header', async () => {
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('sidebar')).toBeInTheDocument()
        expect(screen.getByTestId('header')).toBeInTheDocument()
      })
    })

    it('renders the toaster', async () => {
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('toaster')).toBeInTheDocument()
      })
    })

    it('checks demo mode on mount', async () => {
      renderApp()
      await waitFor(() => {
        expect(mockIsDemoMode).toHaveBeenCalledOnce()
      })
    })

    it('loads DB location on mount', async () => {
      renderApp()
      await waitFor(() => {
        expect(mockGetDbLocation).toHaveBeenCalledOnce()
      })
    })

    it('defaults to dashboard view', async () => {
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('dashboard-view')).toBeInTheDocument()
        expect(screen.getByTestId('current-view')).toHaveTextContent('dashboard')
      })
    })
  })

  // ── Navigation ──

  describe('navigation', () => {
    it('navigates to settings page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => {
        expect(screen.getByTestId('settings-page')).toBeInTheDocument()
        expect(screen.queryByTestId('dashboard-view')).not.toBeInTheDocument()
      })
    })

    it('navigates to fx-rates page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-fx-rates'))
      await user.click(screen.getByTestId('nav-fx-rates'))
      await waitFor(() => {
        expect(screen.getByTestId('fx-rates-page')).toBeInTheDocument()
        expect(screen.queryByTestId('dashboard-view')).not.toBeInTheDocument()
      })
    })

    it('navigates to units page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-units'))
      await user.click(screen.getByTestId('nav-units'))
      await waitFor(() => {
        expect(screen.getByTestId('units-page')).toBeInTheDocument()
        expect(screen.queryByTestId('dashboard-view')).not.toBeInTheDocument()
      })
    })

    it('navigates back to dashboard', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => expect(screen.getByTestId('settings-page')).toBeInTheDocument())

      await user.click(screen.getByTestId('nav-dashboard'))
      await waitFor(() => {
        expect(screen.getByTestId('dashboard-view')).toBeInTheDocument()
        expect(screen.queryByTestId('settings-page')).not.toBeInTheDocument()
      })
    })
  })

  // ── Page title ──

  describe('page title', () => {
    it('shows dashboard title by default', async () => {
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.dashboard')
      })
    })

    it('shows settings title on settings page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => {
        expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.settings')
      })
    })

    it('shows fx-rates title on fx-rates page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-fx-rates'))
      await user.click(screen.getByTestId('nav-fx-rates'))
      await waitFor(() => {
        expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.fxRates')
      })
    })

    it('shows units title on units page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-units'))
      await user.click(screen.getByTestId('nav-units'))
      await waitFor(() => {
        expect(screen.getByTestId('page-title')).toHaveTextContent('sidebar.units')
      })
    })
  })

  // ── Date picker visibility ──

  describe('date picker visibility', () => {
    it('shows date picker on dashboard view', async () => {
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('date-picker-visible')).toBeInTheDocument()
      })
    })

    it('hides date picker on settings page', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => {
        expect(screen.queryByTestId('date-picker-visible')).not.toBeInTheDocument()
      })
    })
  })

  // ── Sidebar collapse ──

  describe('sidebar collapse', () => {
    it('starts expanded (collapsed=false)', async () => {
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('sidebar-collapsed')).toHaveTextContent('false')
      })
    })

    it('toggles collapsed state', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('toggle-collapse'))
      await user.click(screen.getByTestId('toggle-collapse'))
      await waitFor(() => {
        expect(screen.getByTestId('sidebar-collapsed')).toHaveTextContent('true')
      })

      await user.click(screen.getByTestId('toggle-collapse'))
      await waitFor(() => {
        expect(screen.getByTestId('sidebar-collapsed')).toHaveTextContent('false')
      })
    })
  })

  // ── Demo mode banner ──

  describe('demo mode banner', () => {
    it('does not show demo banner when not in demo mode', async () => {
      mockIsDemoMode.mockResolvedValue(false)
      renderApp()
      await waitFor(() => {
        expect(screen.queryByTestId('demo-banner')).not.toBeInTheDocument()
      })
    })

    it('shows demo banner when in demo mode', async () => {
      mockIsDemoMode.mockResolvedValue(true)
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument()
      })
    })

    it('exits demo mode when banner exit is clicked', async () => {
      mockIsDemoMode.mockResolvedValue(true)
      mockExitDemoMode.mockResolvedValue(undefined)
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument()
      })
      await user.click(screen.getByTestId('exit-demo'))
      await waitFor(() => {
        expect(mockExitDemoMode).toHaveBeenCalledOnce()
      })
    })
  })

  // ── Demo mode enter/exit from settings ──

  describe('demo mode enter/exit', () => {
    it('enters demo mode and navigates to dashboard', async () => {
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => screen.getByTestId('enter-demo'))
      await user.click(screen.getByTestId('enter-demo'))

      await waitFor(() => {
        expect(mockEnterDemoMode).toHaveBeenCalledOnce()
      })
    })

    it('exits demo mode and shows success toast', async () => {
      mockIsDemoMode.mockResolvedValue(true)
      const user = userEvent.setup()
      renderApp()

      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument()
      })

      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => screen.getByTestId('exit-demo-settings'))
      await user.click(screen.getByTestId('exit-demo-settings'))

      await waitFor(() => {
        expect(mockExitDemoMode).toHaveBeenCalledOnce()
        expect(mockToastSuccess).toHaveBeenCalledWith('demo.exitedToast')
      })
    })

    it('shows error toast when enter demo mode fails', async () => {
      mockEnterDemoMode.mockRejectedValue(new Error('Demo error'))
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('nav-settings'))
      await user.click(screen.getByTestId('nav-settings'))
      await waitFor(() => screen.getByTestId('enter-demo'))
      await user.click(screen.getByTestId('enter-demo'))

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalledWith('Demo error')
      })
    })

    it('shows error toast when exit demo mode fails', async () => {
      mockIsDemoMode.mockResolvedValue(true)
      mockExitDemoMode.mockRejectedValue(new Error('Exit error'))
      const user = userEvent.setup()
      renderApp()

      await waitFor(() => {
        expect(screen.getByTestId('demo-banner')).toBeInTheDocument()
      })

      await user.click(screen.getByTestId('exit-demo'))

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalledWith('Exit error')
      })
    })
  })

  // ── DB location ──

  describe('DB location', () => {
    it('shows fallback warning toast when DB location has fallbackWarning', async () => {
      mockGetDbLocation.mockResolvedValue({
        ...defaultDbLocation,
        fallbackWarning: true,
      })
      renderApp()
      await waitFor(() => {
        expect(mockToastWarning).toHaveBeenCalledWith('dataStorage.toasts.fallbackWarning')
      })
    })
  })

  // ── Modal rendering ──

  describe('modal rendering', () => {
    it('renders CreateBalanceUpdateModal when modalState is createBalanceUpdate', async () => {
      modalStateValue = { type: 'createBalanceUpdate' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('create-balance-modal')).toBeInTheDocument()
      })
    })

    it('renders EditBalanceUpdateModal when modalState is editBalanceUpdate', async () => {
      modalStateValue = { type: 'editBalanceUpdate', event: makeEvent() }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('edit-balance-modal')).toBeInTheDocument()
      })
    })

    it('renders CreateAccountModal when modalState is createAccount', async () => {
      modalStateValue = { type: 'createAccount', accountType: 'account' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('create-account-modal')).toBeInTheDocument()
        expect(screen.getByTestId('create-account-type')).toHaveTextContent('account')
      })
    })

    it('renders CreateAssetModal when modalState is createAsset', async () => {
      modalStateValue = { type: 'createAsset' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('create-asset-modal')).toBeInTheDocument()
      })
    })

    it('renders EditAccountModal when modalState is editAccount', async () => {
      modalStateValue = {
        type: 'editAccount',
        accountId: 1,
        currentName: 'Test',
        accountType: 'account',
        isCustomUnit: false,
        currencyMinorUnits: 2,
        currentPurchasePriceMinor: null,
        currentPurchaseDate: null,
        currentDepreciationPeriodMonths: null,
      }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('edit-account-modal')).toBeInTheDocument()
      })
    })

    it('renders ConfirmDialog for confirmDeleteAccount', async () => {
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 1,
        name: 'Checking',
        accountType: 'account',
      }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
      })
    })

    it('renders ConfirmDialog for confirmDeleteEvent', async () => {
      modalStateValue = { type: 'confirmDeleteEvent', eventId: 1 }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
      })
    })

    it('renders BulkUpdateBalanceModal when modalState is bulkUpdateBalance', async () => {
      modalStateValue = { type: 'bulkUpdateBalance' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('bulk-update-modal')).toBeInTheDocument()
      })
    })

    it('renders ConfirmDialog for fetchFxRatePrompt', async () => {
      modalStateValue = { type: 'fetchFxRatePrompt', date: '2026-01-15' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('confirm-dialog')).toBeInTheDocument()
      })
    })

    it('renders UpdateAssetValueModal when modalState is updateAssetValue', async () => {
      modalStateValue = {
        type: 'updateAssetValue',
        accountId: 20,
        accountName: 'House',
        currencyCode: 'EUR',
        currencyMinorUnits: 2,
        isCustomUnit: false,
        balanceMinor: 30000000,
      }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('update-asset-modal')).toBeInTheDocument()
      })
    })

    it('renders ReorderModal for reorderAccounts', async () => {
      setupDefaultMocks({ snapshot: [makeSnapshot()] })
      modalStateValue = { type: 'reorderAccounts' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('reorder-modal')).toBeInTheDocument()
        expect(screen.getByTestId('reorder-title')).toHaveTextContent('reorder.titleAccounts')
      })
    })

    it('renders ReorderModal for reorderBuckets', async () => {
      setupDefaultMocks({ snapshot: [makeBucket()] })
      modalStateValue = { type: 'reorderBuckets' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('reorder-modal')).toBeInTheDocument()
        expect(screen.getByTestId('reorder-title')).toHaveTextContent('reorder.titleBuckets')
      })
    })

    it('renders ReorderModal for reorderAssets', async () => {
      setupDefaultMocks({ snapshot: [makeAsset()] })
      modalStateValue = { type: 'reorderAssets' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('reorder-modal')).toBeInTheDocument()
        expect(screen.getByTestId('reorder-title')).toHaveTextContent('reorder.titleAssets')
      })
    })

    it('renders ManageLinkedAssetsModal when modalState is manageLinkedAssets', async () => {
      modalStateValue = { type: 'manageLinkedAssets', accountId: 1, accountName: 'Checking' }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('manage-linked-assets-modal')).toBeInTheDocument()
      })
    })

    it('does not render any modal when modalState is none', async () => {
      modalStateValue = { type: 'none' }
      renderApp()
      await waitFor(() => {
        expect(screen.queryByTestId('create-balance-modal')).not.toBeInTheDocument()
        expect(screen.queryByTestId('edit-balance-modal')).not.toBeInTheDocument()
        expect(screen.queryByTestId('create-account-modal')).not.toBeInTheDocument()
        expect(screen.queryByTestId('confirm-dialog')).not.toBeInTheDocument()
        expect(screen.queryByTestId('bulk-update-modal')).not.toBeInTheDocument()
        expect(screen.queryByTestId('reorder-modal')).not.toBeInTheDocument()
      })
    })
  })

  // ── ConfirmDialog behavior ──

  describe('confirm dialog behavior', () => {
    it('calls handleDeleteAccount when confirmDeleteAccount is confirmed', async () => {
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 42,
        name: 'OldAccount',
        accountType: 'account',
      }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))
      await waitFor(() => {
        expect(mockHandleDeleteAccount).toHaveBeenCalledWith(42)
      })
    })

    it('calls closeModal when confirmDeleteAccount is cancelled', async () => {
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 42,
        name: 'OldAccount',
        accountType: 'account',
      }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-cancel'))
      await user.click(screen.getByTestId('confirm-cancel'))
      await waitFor(() => {
        expect(mockCloseModal).toHaveBeenCalledOnce()
      })
    })

    it('calls handleDeleteEvent when confirmDeleteEvent is confirmed', async () => {
      modalStateValue = { type: 'confirmDeleteEvent', eventId: 99 }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))
      await waitFor(() => {
        expect(mockHandleDeleteEvent).toHaveBeenCalledWith(99)
      })
    })

    it('shows asset deletion warning for linked assets', async () => {
      const acctLinkedToAsset = makeSnapshot({
        accountId: 1,
        accountName: 'Investment Account',
        linkedAssetIds: [20],
      })
      setupDefaultMocks({ snapshot: [acctLinkedToAsset] })
      modalStateValue = {
        type: 'confirmDeleteAccount',
        accountId: 20,
        name: 'House',
        accountType: 'asset',
      }
      renderApp()
      await waitFor(() => {
        expect(screen.getByTestId('confirm-message')).toHaveTextContent('modals.confirm.deleteAssetWithLinks')
      })
    })
  })

  // ── CreateAssetModal onSuccess ──

  describe('CreateAssetModal onSuccess', () => {
    it('calls closeModal when asset is created', async () => {
      modalStateValue = { type: 'createAsset' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('create-asset-success'))
      await user.click(screen.getByTestId('create-asset-success'))

      await waitFor(() => {
        expect(mockCloseModal).toHaveBeenCalledOnce()
      })
    })
  })

  // ── FxRate prompt modal ──

  describe('fetchFxRatePrompt modal', () => {
    it('fetches FX rates and closes on confirm', async () => {
      modalStateValue = { type: 'fetchFxRatePrompt', date: '2026-01-15' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))

      await waitFor(() => {
        expect(mockFetchFxRates).toHaveBeenCalledWith('2026-01-15')
        expect(mockCloseModal).toHaveBeenCalled()
      })
    })

    it('closes modal even when fetch fails', async () => {
      mockFetchFxRates.mockRejectedValue(new Error('fetch fail'))
      modalStateValue = { type: 'fetchFxRatePrompt', date: '2026-01-15' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))

      await waitFor(() => {
        expect(mockCloseModal).toHaveBeenCalled()
      })
    })
  })

  // ── ConfirmSwitchDb modal ──

  describe('confirmSwitchDb modal', () => {
    it('calls changeDbLocation with switch action on confirm', async () => {
      modalStateValue = { type: 'confirmSwitchDb', folder: '/existing/db' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))

      await waitFor(() => {
        expect(mockChangeDbLocation).toHaveBeenCalledWith('/existing/db', 'switch')
        expect(mockToastSuccess).toHaveBeenCalled()
        expect(mockCloseModal).toHaveBeenCalled()
      })
    })

    it('shows error toast when changeDbLocation fails', async () => {
      mockChangeDbLocation.mockRejectedValue(new Error('switch fail'))
      modalStateValue = { type: 'confirmSwitchDb', folder: '/existing/db' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled()
      })
    })
  })

  // ── DbLocationChoice modal ──

  describe('dbLocationChoice modal', () => {
    it('calls changeDbLocation with move action', async () => {
      modalStateValue = { type: 'dbLocationChoice', folder: '/new/path', isReset: false }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('db-choice-move'))
      await user.click(screen.getByTestId('db-choice-move'))

      await waitFor(() => {
        expect(mockChangeDbLocation).toHaveBeenCalledWith('/new/path', 'move')
        expect(mockToastSuccess).toHaveBeenCalled()
        expect(mockCloseModal).toHaveBeenCalled()
      })
    })

    it('calls changeDbLocation with fresh action', async () => {
      modalStateValue = { type: 'dbLocationChoice', folder: '/new/path', isReset: false }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('db-choice-fresh'))
      await user.click(screen.getByTestId('db-choice-fresh'))

      await waitFor(() => {
        expect(mockChangeDbLocation).toHaveBeenCalledWith('/new/path', 'fresh')
        expect(mockToastSuccess).toHaveBeenCalled()
      })
    })

    it('calls resetDbLocation when isReset is true', async () => {
      modalStateValue = { type: 'dbLocationChoice', folder: '', isReset: true }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('db-choice-move'))
      await user.click(screen.getByTestId('db-choice-move'))

      await waitFor(() => {
        expect(mockResetDbLocation).toHaveBeenCalledWith('move')
        expect(mockToastSuccess).toHaveBeenCalled()
      })
    })

    it('shows error toast when dbLocationChoice action fails (non-reset)', async () => {
      mockChangeDbLocation.mockRejectedValue(new Error('change fail'))
      modalStateValue = { type: 'dbLocationChoice', folder: '/new/path', isReset: false }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('db-choice-move'))
      await user.click(screen.getByTestId('db-choice-move'))

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled()
      })
    })

    it('shows error toast when dbLocationChoice reset action fails', async () => {
      mockResetDbLocation.mockRejectedValue(new Error('reset fail'))
      modalStateValue = { type: 'dbLocationChoice', folder: '', isReset: true }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('db-choice-move'))
      await user.click(screen.getByTestId('db-choice-move'))

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled()
      })
    })
  })

  // ── ConfirmResetDbLocation modal ──

  describe('confirmResetDbLocation modal', () => {
    it('calls resetDbLocation with switch on confirm', async () => {
      modalStateValue = { type: 'confirmResetDbLocation' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))

      await waitFor(() => {
        expect(mockResetDbLocation).toHaveBeenCalledWith('switch')
        expect(mockToastSuccess).toHaveBeenCalled()
        expect(mockCloseModal).toHaveBeenCalled()
      })
    })

    it('shows error toast when confirmResetDbLocation fails', async () => {
      mockResetDbLocation.mockRejectedValue(new Error('reset fail'))
      modalStateValue = { type: 'confirmResetDbLocation' }
      const user = userEvent.setup()
      renderApp()
      await waitFor(() => screen.getByTestId('confirm-ok'))
      await user.click(screen.getByTestId('confirm-ok'))

      await waitFor(() => {
        expect(mockToastError).toHaveBeenCalled()
      })
    })
  })
})
