import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor, act, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import DashboardView from './DashboardView'
import { SelectedPersonProvider } from '../../app/SelectedPersonContext'
import type { AsOf, EventWithData, Currency, Metric, RateWarning, SnapshotRow } from '../../shared/types'
import { LIVE } from '../../shared/utils/snapshot'
import { makeCountedLine, makeMetric, makeMoney, makeRateWarning, makeRow, makeSnapshot } from '../../shared/testing/snapshotFixtures'

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
  listPersons: vi.fn(),
}))

import { getAccountsSnapshot, listEvents, getConsolidationCurrency, listPersons } from '../../shared/api'

// ── Mock contexts ─────────────────────────────────────────────────────────────
const mockSetModalState = vi.fn()
vi.mock('../../app/useModal', () => ({
  useModal: () => ({ setModalState: mockSetModalState, closeModal: vi.fn(), modalState: { type: 'none' } }),
}))

const demoModeState = { isDemoMode: false }
const mockOnEnterDemoMode = vi.fn()
vi.mock('../../app/useDemo', () => ({
  useDemo: () => ({ isDemoMode: demoModeState.isDemoMode, onEnterDemoMode: mockOnEnterDemoMode, onExitDemoMode: vi.fn() }),
}))

const timeMachine: { asOf: AsOf; travelTo: Mock; backToLive: Mock } = { asOf: LIVE, travelTo: vi.fn(), backToLive: vi.fn() }
vi.mock('../../app/useTimeMachine', () => ({
  useTimeMachine: () => timeMachine,
}))

// ── Mock i18n — pass-through that returns the key, with interpolation values appended ─────
vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

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
  default: (props: { events: EventWithData[]; onEditEvent: (event: EventWithData) => void; onAddEvents: () => void; onViewAll?: () => void }) => (
    <div data-testid="ledger">
      <button data-testid="edit-first-event-btn" disabled={props.events.length === 0} onClick={() => props.onEditEvent(props.events[0])}>
        Edit
      </button>
      <button data-testid="add-events-btn" onClick={props.onAddEvents}>
        Add
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

const makeBucket = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  return makeRow({
    accountId: 10,
    accountName: 'Vacation',
    accountType: 'bucket',
    balanceMinor: 50000,
    ...overrides,
  })
}

const makeAsset = (overrides?: Partial<SnapshotRow>): SnapshotRow => {
  return makeRow({
    accountId: 20,
    accountName: 'House',
    accountType: 'asset',
    balanceMinor: 30000000,
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
    vatReclaimablePctBps: null,
    expenseDeductiblePctBps: null,
    prepaidUntil: null,
    isLinkedToTaxable: false,
    linkedTaxableEventId: null,
    hasLinkedCashflows: false,
    linkedCashflowCount: 0,
    linkedAssetId: null,
    linkedPrepaidEventId: null,
    isSystemGenerated: false,
    reclaimedVat: null,
    ...overrides,
  }
}

const setupApiMocks = (options?: { snapshot?: SnapshotRow[]; metrics?: Metric[]; rateWarnings?: RateWarning[]; events?: EventWithData[] }) => {
  ;(getAccountsSnapshot as Mock).mockResolvedValue(
    makeSnapshot({
      rows: options?.snapshot ?? [makeRow()],
      metrics: options?.metrics ?? [makeMetric('totalBalance')],
      rateWarnings: options?.rateWarnings ?? [],
    }),
  )
  ;(listEvents as Mock).mockResolvedValue({ events: options?.events ?? [makeEvent()], totalCount: options?.events?.length ?? 1 })
  ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
  ;(listPersons as Mock).mockResolvedValue([])
}

const renderDashboard = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  return render(
    <QueryClientProvider client={queryClient}>
      <SelectedPersonProvider>
        <DashboardView />
      </SelectedPersonProvider>
    </QueryClientProvider>,
  )
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('DashboardView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    demoModeState.isDemoMode = false
    timeMachine.asOf = LIVE
    setupApiMocks()
  })

  // ── Metric rendering ──

  describe('metrics', () => {
    const metricCards = () => screen.getAllByRole('button').filter((b) => b.getAttribute('aria-haspopup') === 'dialog')

    it('renders the metrics in the order Rust returns them, labeled by kind', async () => {
      setupApiMocks({
        snapshot: [makeRow(), makeBucket(), makeAsset()],
        metrics: [makeMetric('netWorth'), makeMetric('liquid'), makeMetric('leftToSpend')],
      })
      renderDashboard()
      await waitFor(() => expect(metricCards()).toHaveLength(3))
      expect(metricCards().map((card) => within(card).getByText(/^metrics\./).textContent)).toEqual(['metrics.netWorth', 'metrics.liquid', 'metrics.leftToSpend'])
    })

    it('shows only the metrics Rust returns', async () => {
      setupApiMocks({ metrics: [makeMetric('totalBalance')] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument()
      })
      expect(screen.queryByText('metrics.netWorth')).not.toBeInTheDocument()
      expect(screen.queryByText('metrics.leftToSpend')).not.toBeInTheDocument()
    })

    it('shows the total Rust returned even when it differs from the sum of the rows', async () => {
      setupApiMocks({
        snapshot: [makeRow({ accountId: 1, balanceMinor: 100000 }), makeRow({ accountId: 2, accountName: 'Savings', balanceMinor: 250000 })],
        metrics: [makeMetric('totalBalance', { totalMinor: 777 })],
      })
      renderDashboard()
      await waitFor(() => expect(metricCards()).toHaveLength(1))
      expect(within(metricCards()[0]).getByTestId('number-value')).toHaveTextContent('777')
    })
  })

  // ── Metric breakdown ──

  describe('metric breakdown', () => {
    const netWorth = makeMetric('netWorth', { counted: [makeCountedLine({ accountName: 'Checking' })] })
    const liquid = makeMetric('liquid', { counted: [makeCountedLine({ accountId: 3, accountName: 'Savings', signedMinor: 250000 })] })

    it('opens the breakdown of the metric that was clicked', async () => {
      const user = userEvent.setup()
      setupApiMocks({ metrics: [netWorth, liquid] })
      renderDashboard()
      await user.click(await screen.findByRole('button', { name: /metrics\.liquid/ }))

      const dialog = await screen.findByRole('dialog')
      expect(within(dialog).getByText('metrics.liquid')).toBeInTheDocument()
      expect(within(dialog).getByText('Savings')).toBeInTheDocument()
      expect(within(dialog).queryByText('Checking')).not.toBeInTheDocument()
    })

    it('opens from the keyboard and closes with Escape', async () => {
      const user = userEvent.setup()
      setupApiMocks({ metrics: [netWorth] })
      renderDashboard()
      const card = await screen.findByRole('button', { name: /metrics\.netWorth/ })
      card.focus()
      await user.keyboard('{Enter}')
      expect(await screen.findByRole('dialog')).toBeInTheDocument()

      await user.keyboard('{Escape}')
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    })

    it('does not open a breakdown until a metric is clicked', async () => {
      setupApiMocks({ metrics: [netWorth] })
      renderDashboard()
      await screen.findByRole('button', { name: /metrics\.netWorth/ })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  // ── Time machine ──

  describe('time machine', () => {
    it('asks for the live snapshot and shows no banner by default', async () => {
      renderDashboard()
      await waitFor(() => expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument())
      expect(getAccountsSnapshot).toHaveBeenCalledWith({ kind: 'live' }, undefined)
      expect(screen.queryByRole('status')).not.toBeInTheDocument()
    })

    it('shows the picked date in a banner and asks for the snapshot at that day', async () => {
      timeMachine.asOf = { kind: 'at', until: '2026-10-20' }
      renderDashboard()
      const banner = await screen.findByRole('status')
      expect(banner).toHaveTextContent('timeMachine.viewing (date=20 October 2026)')
      expect(getAccountsSnapshot).toHaveBeenCalledWith({ kind: 'at', until: '2026-10-20' }, undefined)
    })

    it('goes back to live from the banner', async () => {
      const user = userEvent.setup()
      timeMachine.asOf = { kind: 'at', until: '2026-10-20' }
      renderDashboard()
      await user.click(await screen.findByRole('button', { name: 'timeMachine.backToLive' }))
      expect(timeMachine.backToLive).toHaveBeenCalledOnce()
    })
  })

  // ── Rate warnings ──

  describe('rate warnings', () => {
    it('lists one warning per entry with the account name and amount', async () => {
      setupApiMocks({
        rateWarnings: [
          makeRateWarning({ accountId: 2, accountName: 'Dollar account', native: makeMoney(5000, 'USD') }),
          makeRateWarning({
            accountId: 3,
            accountName: 'Pound account',
            native: makeMoney(7000, 'GBP'),
            problem: { kind: 'stale', limitDays: 7, rate: { mantissa: 8, exponent: -1, isDirect: false, date: '2026-10-12', isManual: false, ageDays: 13, stale: true } },
          }),
        ],
      })
      renderDashboard()
      const items = await screen.findAllByRole('listitem')
      expect(items).toHaveLength(2)
      expect(items[0]).toHaveTextContent('Dollar account (5000) rateWarnings.noRate (currency=USD)')
      expect(items[1]).toHaveTextContent('Pound account (7000) rateWarnings.stale (currency=GBP, date=12 October 2026, count=13)')
    })

    it('shows no warnings when Rust sends none', async () => {
      renderDashboard()
      await waitFor(() => expect(screen.getByText('metrics.totalBalance')).toBeInTheDocument())
      expect(screen.queryByRole('listitem')).not.toBeInTheDocument()
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

    it('does not show empty state when the snapshot holds only an asset', async () => {
      setupApiMocks({ snapshot: [makeAsset()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-assets.sectionTitle')).toBeInTheDocument()
      })
      expect(screen.queryByText('demo.emptyTitle')).not.toBeInTheDocument()
      expect(screen.getByTestId('account-cards-buckets.sectionTitle')).toBeInTheDocument()
      expect(screen.getByTestId('ledger')).toBeInTheDocument()
    })

    it('renders nothing instead of the empty state while the snapshot is loading', async () => {
      ;(getAccountsSnapshot as Mock).mockReturnValue(new Promise(() => {}))
      renderDashboard()
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0))
      })
      expect(getAccountsSnapshot).toHaveBeenCalled()
      expect(screen.queryByText('demo.emptyTitle')).not.toBeInTheDocument()
      expect(screen.queryByTestId('account-cards-accounts.sectionTitle')).not.toBeInTheDocument()
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
      setupApiMocks({ snapshot: [makeRow()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-accounts.sectionTitle')).toBeInTheDocument()
      })
    })

    it('renders AccountCards for buckets when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeRow(), makeBucket()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('account-cards-buckets.sectionTitle')).toBeInTheDocument()
      })
    })

    it('renders AccountCards for assets when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeRow(), makeAsset()] })
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
      setupApiMocks({ snapshot: [makeRow()] })
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
      setupApiMocks({ snapshot: [makeRow(), makeBucket()] })
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
      setupApiMocks({ snapshot: [makeRow(), makeAsset()] })
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
      setupApiMocks({ snapshot: [makeRow()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('reorder-btn-accounts.sectionTitle'))
      await user.click(screen.getByTestId('reorder-btn-accounts.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'reorderAccounts' })
    })

    it('opens reorderBuckets modal', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeRow(), makeBucket()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('reorder-btn-buckets.sectionTitle'))
      await user.click(screen.getByTestId('reorder-btn-buckets.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'reorderBuckets' })
    })

    it('opens reorderAssets modal', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeRow(), makeAsset()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('reorder-btn-assets.sectionTitle'))
      await user.click(screen.getByTestId('reorder-btn-assets.sectionTitle'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'reorderAssets' })
    })
  })

  // ── Ledger section ──

  describe('ledger', () => {
    it('renders Ledger when accounts exist', async () => {
      setupApiMocks({ snapshot: [makeRow()] })
      renderDashboard()
      await waitFor(() => {
        expect(screen.getByTestId('ledger')).toBeInTheDocument()
      })
    })

    it('opens addEvents modal from Ledger', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeRow()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('add-events-btn'))
      await user.click(screen.getByTestId('add-events-btn'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'addEvents' })
    })

    it('opens the taxable event editor for a prepaid expense', async () => {
      const user = userEvent.setup()
      const prepaid = makeEvent({ id: 7, eventType: 'prepaid_expense' })
      setupApiMocks({ snapshot: [makeRow()], events: [prepaid] })
      renderDashboard()
      await waitFor(() => expect(screen.getByTestId('edit-first-event-btn')).toBeEnabled())
      await user.click(screen.getByTestId('edit-first-event-btn'))
      expect(mockSetModalState).toHaveBeenCalledWith({ type: 'editTaxableEvent', event: prepaid })
    })

    it('navigates to ledger when View all is clicked', async () => {
      const user = userEvent.setup()
      setupApiMocks({ snapshot: [makeRow()] })
      renderDashboard()
      await waitFor(() => screen.getByTestId('view-all-btn'))
      await user.click(screen.getByTestId('view-all-btn'))
      expect(mockNavigate).toHaveBeenCalledWith({ to: '/ledger' })
    })
  })

  // ── MetricCard styling ──

  describe('MetricCard styling', () => {
    it('applies destructive class for negative values', async () => {
      setupApiMocks({ metrics: [makeMetric('totalBalance', { totalMinor: -5000 })] })
      renderDashboard()
      const card = await screen.findByRole('button', { name: /metrics\.totalBalance/ })
      expect(card.querySelector('.text-destructive')).toBeInTheDocument()
    })

    it('does not apply destructive class for positive values', async () => {
      setupApiMocks({ metrics: [makeMetric('totalBalance', { totalMinor: 5000 })] })
      renderDashboard()
      const card = await screen.findByRole('button', { name: /metrics\.totalBalance/ })
      expect(card.querySelector('.text-destructive')).not.toBeInTheDocument()
    })
  })
})
