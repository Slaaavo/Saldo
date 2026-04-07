import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import DashboardView from './DashboardView'
import type { SnapshotRow, EventWithData, Currency } from '../../shared/types'

// ── Mock TanStack Router ──────────────────────────────────────────────────────
const mockNavigate = vi.fn()
vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}))

// ── Mock API layer ────────────────────────────────────────────────────────────
vi.mock('../../shared/api', () => ({
  getAccountsSnapshot: vi.fn(),
  listEvents: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  getEventById: vi.fn(),
}))

import { getAccountsSnapshot, listEvents, getConsolidationCurrency } from '../../shared/api'

// ── Mock contexts ─────────────────────────────────────────────────────────────
const mockSetModalState = vi.fn()
vi.mock('../../app/ModalContext', () => ({
  useModal: () => ({ setModalState: mockSetModalState, closeModal: vi.fn(), modalState: { type: 'none' } }),
}))

const demoModeState = { isDemoMode: false }
const mockOnEnterDemoMode = vi.fn()
vi.mock('../../app/DemoContext', () => ({
  useDemo: () => ({ isDemoMode: demoModeState.isDemoMode, onEnterDemoMode: mockOnEnterDemoMode, onExitDemoMode: vi.fn() }),
}))

vi.mock('../../app/SelectedDateContext', () => ({
  useSelectedDate: () => ({ selectedDate: '2026-01-15', setSelectedDate: vi.fn() }),
}))

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

// ── Mock child components to isolate DashboardView logic ────────────────────
vi.mock('../../shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

vi.mock('./AccountCards', () => ({
  default: (props: { sectionTitle?: string; addButtonLabel?: string; emptyMessage?: string; onCreateAccount: () => void; onReorder?: () => void; snapshot: SnapshotRow[] }) => (
    <div data-testid={`account-cards-${props.sectionTitle ?? 'unknown'}`}>
      <span data-testid="section-title">{props.sectionTitle}</span>
      {props.snapshot.length === 0 && props.emptyMessage && <span data-testid="empty-message">{props.emptyMessage}</span>}
      <button data-testid={`add-btn-${props.sectionTitle}`} onClick={props.onCreateAccount}>
        {props.addButtonLabel}
      </button>
      {props.onReorder && (
        <button data-testid={`reorder-btn-${props.sectionTitle}`} onClick={props.onReorder}>
          reorder
        </button>
      )}
    </div>
  ),
}))

vi.mock('./Ledger', () => ({
  default: (props: { onUpdateBalances: () => void; onViewAll?: () => void }) => (
    <div data-testid="ledger">
      <button data-testid="update-balances-btn" onClick={props.onUpdateBalances}>
        Update
      </button>
      {props.onViewAll && (
        <button data-testid="view-all-btn" onClick={props.onViewAll}>
          View all
        </button>
      )}
    </div>
  ),
}))

// ── Helpers ─────────────────────────────────────────────────────────────────
const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

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
    ...overrides,
  }
}

const setupApiMocks = (options?: { snapshot?: SnapshotRow[]; events?: EventWithData[] }) => {
  ;(getAccountsSnapshot as Mock).mockResolvedValue(options?.snapshot ?? [makeSnapshot()])
  ;(listEvents as Mock).mockResolvedValue({ events: options?.events ?? [makeEvent()], totalCount: options?.events?.length ?? 1 })
  ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
}

const renderDashboard = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  return render(
    <QueryClientProvider client={queryClient}>
      <DashboardView />
    </QueryClientProvider>,
  )
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('DashboardView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    demoModeState.isDemoMode = false
    setupApiMocks()
  })

  // ── Metric rendering ──

  describe('metrics', () => {
    it('shows totalBalance metric when there are no assets', async () => {
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument()
      })
      expect(screen.queryByText('metrics.netWorth')).not.toBeInTheDocument()
      expect(screen.queryByText('metrics.liquid')).not.toBeInTheDocument()
    })

    it('shows netWorth and liquid metrics when there are assets', async () => {
      setupApiMocks({ snapshot: [makeSnapshot(), makeAsset()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.netWorth')).toBeInTheDocument()
        expect(screen.getByText('metrics.liquid')).toBeInTheDocument()
      })
      expect(screen.queryByText('metrics.totalBalance')).not.toBeInTheDocument()
    })

    it('shows leftToSpend metric when buckets exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot(), makeBucket()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.leftToSpend')).toBeInTheDocument()
      })
    })

    it('hides leftToSpend metric when no buckets exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument()
      })
      expect(screen.queryByText('metrics.leftToSpend')).not.toBeInTheDocument()
    })

    it('shows all three metrics when assets and buckets exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot(), makeBucket(), makeAsset()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.netWorth')).toBeInTheDocument()
        expect(screen.getByText('metrics.liquid')).toBeInTheDocument()
        expect(screen.getByText('metrics.leftToSpend')).toBeInTheDocument()
      })
    })

    it('renders metric values via NumberValue', async () => {
      setupApiMocks({ snapshot: [makeSnapshot({ convertedBalanceMinor: 123456, isLinkedToAsset: false })] })
      renderDashboard()
      await waitFor(() => {
        const values = screen.getAllByTestId('number-value')
        expect(values.some((el) => el.textContent === '123456')).toBe(true)
      })
    })
  })

  // ── FX rate missing warning ──

  describe('FX rate missing warning', () => {
    it('shows warning when missingFxCurrencies is non-empty', async () => {
      setupApiMocks({ snapshot: [makeSnapshot({ fxRateMissing: true, currencyCode: 'USD' })] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.fxRateMissing', { exact: false })).toBeInTheDocument()
      })
    })

    it('hides warning when missingFxCurrencies is empty', async () => {
      setupApiMocks({ snapshot: [makeSnapshot({ fxRateMissing: false })] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument()
      })
      expect(screen.queryByText('metrics.fxRateMissing')).not.toBeInTheDocument()
    })
  })

  // ── Empty state / demo mode ──

  describe('empty state', () => {
    it('shows empty state when no accounts and not in demo mode', async () => {
      setupApiMocks({ snapshot: [] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('demo.emptyTitle')).toBeInTheDocument()
        expect(screen.getByText('demo.emptyDesc')).toBeInTheDocument()
        expect(screen.getByText('demo.emptyCta')).toBeInTheDocument()
        expect(screen.getByText('accounts.addAccount')).toBeInTheDocument()
      })
    })

    it('calls onEnterDemoMode when demo CTA is clicked', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [] })
      renderDashboard()
      await waitFor(() => screen.getByText('demo.emptyCta'))
      await user.click(screen.getByText('demo.emptyCta'))
      expect(mockOnEnterDemoMode).toHaveBeenCalledOnce()
    })

    it('opens createAccount modal when add account button is clicked in empty state', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [] })
      renderDashboard()
      await waitFor(() => screen.getByText('accounts.addAccount'))
      await user.click(screen.getByText('accounts.addAccount'))
      expect(mockSetModalState).toHaveBeenCalledWith({
        type: 'createAccount',
        accountType: 'account',
      })
    })

    it('does not show empty state when in demo mode with no accounts', async () => {
      demoModeState.isDemoMode = true
      setupApiMocks({ snapshot: [] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.queryByText('demo.emptyTitle')).not.toBeInTheDocument()
      })
    })
  })

  // ── Account cards section ──

  describe('account cards section', () => {
    it('renders AccountCards for accounts when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-accounts.sectionTitle')).toBeInTheDocument()
      })
    })

    it('renders AccountCards for buckets when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot(), makeBucket()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-buckets.sectionTitle')).toBeInTheDocument()
      })
    })

    it('renders AccountCards for assets when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot(), makeAsset()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-assets.sectionTitle')).toBeInTheDocument()
      })
    })

    it('does not render buckets, assets, or ledger sections when no accounts exist', async () => {
      demoModeState.isDemoMode = true
      setupApiMocks({ snapshot: [] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-accounts.sectionTitle')).toBeInTheDocument()
      })
      expect(screen.queryByTestId('account-cards-buckets.sectionTitle')).not.toBeInTheDocument()
      expect(screen.queryByTestId('account-cards-assets.sectionTitle')).not.toBeInTheDocument()
      expect(screen.queryByTestId('ledger')).not.toBeInTheDocument()
    })
  })

  // ── Create account/bucket/asset actions ──

  describe('section add/create buttons', () => {
    it('opens createAccount modal for accounts section', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('add-btn-accounts.sectionTitle'))
      await user.click(screen.getByTestId('add-btn-accounts.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({
        type: 'createAccount',
        accountType: 'account',
      })
    })

    it('opens createAccount modal for buckets section', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot(), makeBucket()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('add-btn-buckets.sectionTitle'))
      await user.click(screen.getByTestId('add-btn-buckets.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({
        type: 'createAccount',
        accountType: 'bucket',
      })
    })

    it('opens createAsset modal for assets section', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot(), makeAsset()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('add-btn-assets.sectionTitle'))
      await user.click(screen.getByTestId('add-btn-assets.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'createAsset' })
    })
  })

  // ── Reorder actions ──

  describe('reorder buttons', () => {
    it('opens reorderAccounts modal', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('reorder-btn-accounts.sectionTitle'))
      await user.click(screen.getByTestId('reorder-btn-accounts.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'reorderAccounts' })
    })

    it('opens reorderBuckets modal', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot(), makeBucket()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('reorder-btn-buckets.sectionTitle'))
      await user.click(screen.getByTestId('reorder-btn-buckets.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'reorderBuckets' })
    })

    it('opens reorderAssets modal', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot(), makeAsset()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('reorder-btn-assets.sectionTitle'))
      await user.click(screen.getByTestId('reorder-btn-assets.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'reorderAssets' })
    })
  })

  // ── Ledger section ──

  describe('ledger', () => {
    it('renders Ledger when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('ledger')).toBeInTheDocument()
      })
    })

    it('opens bulkUpdateBalance modal from Ledger', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('update-balances-btn'))
      await user.click(screen.getByTestId('update-balances-btn'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'bulkUpdateBalance' })
    })

    it('navigates to ledger when View all is clicked', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeSnapshot()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('view-all-btn'))
      await user.click(screen.getByTestId('view-all-btn'))
      expect(mockNavigate).toHaveBeenCalledWith({ to: '/ledger' })
    })
  })

  // ── MetricCard styling ──

  describe('MetricCard styling', () => {
    it('applies destructive class for negative values', async () => {
      setupApiMocks({ snapshot: [makeSnapshot({ convertedBalanceMinor: -5000, isLinkedToAsset: false })] })
      renderDashboard()
      await waitFor(() => {
        const metricEl = screen.getByText('metrics.totalBalance').closest('div')
        const valueEl = metricEl?.querySelector('.text-destructive')
        expect(valueEl).toBeInTheDocument()
      })
    })

    it('does not apply destructive class for positive values', async () => {
      setupApiMocks({ snapshot: [makeSnapshot({ convertedBalanceMinor: 5000, isLinkedToAsset: false })] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument()
      })
      const metricEl = screen.getByText('metrics.totalBalance').closest('div')
      const valueEl = metricEl?.querySelector('.text-destructive')
      expect(valueEl).not.toBeInTheDocument()
    })
  })
})
