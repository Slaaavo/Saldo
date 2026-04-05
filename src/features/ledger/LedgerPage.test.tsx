import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import LedgerPage from './LedgerPage'

// ── Mock i18n ─────────────────────────────────────────────────────────────
vi.mock('react-i18next', () => {
  // Stable `t` reference prevents useCallback deps from changing on every render
  const t = (key: string, opts?: Record<string, unknown>) => {
    if (opts) {
      let result = key
      for (const [k, v] of Object.entries(opts)) {
        result = result.replace(`{{${k}}}`, String(v))
      }
      return result
    }
    return key
  }
  return {
    useTranslation: () => ({ t, i18n: { language: 'en' } }),
  }
})

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../../shared/api', () => ({
  listEvents: vi.fn(),
  getAccountsSnapshot: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  getEventById: vi.fn(),
}))

// ── Mock ModalContext ───────────────────────────────────────────────────────
const mockSetModalState = vi.fn()
vi.mock('../../app/ModalContext', () => ({
  useModal: () => ({ setModalState: mockSetModalState, closeModal: vi.fn(), modalState: { type: 'none' } }),
}))

// ── Mock child components ──────────────────────────────────────────────────
vi.mock('../../shared/ui/LedgerEventList', () => ({
  default: (props: { events: unknown[]; onEditEvent: (e: unknown) => void; onDeleteEvent: (id: number) => void }) => (
    <div data-testid="ledger-event-list">
      <span data-testid="event-count">{props.events.length}</span>
      <button
        data-testid="edit-btn"
        onClick={() =>
          props.onEditEvent({
            id: 1,
            accountId: 1,
            accountName: 'Test',
            accountType: 'account',
            eventType: 'balance_update',
            eventDate: '2026-01-01T00:00:00',
            amountMinor: 0,
            note: null,
            createdAt: '2026-01-01T00:00:00',
            currencyCode: 'EUR',
            currencyMinorUnits: 2,
          })
        }
      >
        Edit
      </button>
      <button data-testid="delete-btn" onClick={() => props.onDeleteEvent(42)}>
        Delete
      </button>
    </div>
  ),
}))

vi.mock('./PortfolioItemFilter', () => ({
  default: (props: { onChange: (ids: number[]) => void }) => (
    <div data-testid="portfolio-filter">
      <button data-testid="select-account-btn" onClick={() => props.onChange([5])}>
        Select
      </button>
    </div>
  ),
}))

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: (props: { value?: string; onChange: (d: string) => void; placeholder?: string }) => (
    <input data-testid={`date-picker-${props.placeholder}`} value={props.value ?? ''} onChange={(e) => props.onChange(e.target.value)} placeholder={props.placeholder} readOnly />
  ),
}))

import { listEvents, getAccountsSnapshot, getConsolidationCurrency } from '../../shared/api'

const renderLedgerPage = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  return render(
    <QueryClientProvider client={queryClient}>
      <LedgerPage />
    </QueryClientProvider>,
  )
}

describe('LedgerPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listEvents as Mock).mockResolvedValue({ events: [], totalCount: 0 })
    ;(getAccountsSnapshot as Mock).mockResolvedValue([])
    ;(getConsolidationCurrency as Mock).mockResolvedValue({ id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false })
  })

  // ── Rendering ─────────────────────────────────────────────────────────────

  it('renders the page title', () => {
    renderLedgerPage()
    expect(screen.getByText('ledgerPage.title')).toBeInTheDocument()
  })

  it('renders the filter bar with From and To date pickers and portfolio filter', () => {
    renderLedgerPage()
    expect(screen.getByTestId('date-picker-ledgerPage.filterFrom')).toBeInTheDocument()
    expect(screen.getByTestId('date-picker-ledgerPage.filterTo')).toBeInTheDocument()
    expect(screen.getByTestId('portfolio-filter')).toBeInTheDocument()
  })

  it('renders the Update Balances button', () => {
    renderLedgerPage()
    expect(screen.getByText('ledger.updateBalances')).toBeInTheDocument()
  })

  // ── Modal dispatch ────────────────────────────────────────────────────────

  it('dispatches bulkUpdateBalance modal when Update Balances is clicked', async () => {
    const user = userEvent.setup()
    renderLedgerPage()
    await user.click(screen.getByText('ledger.updateBalances'))
    expect(mockSetModalState).toHaveBeenCalledWith({ type: 'bulkUpdateBalance' })
  })

  it('dispatches editBalanceUpdate modal when edit button is clicked', async () => {
    const user = userEvent.setup()
    ;(listEvents as Mock).mockResolvedValue({
      events: [
        {
          id: 1,
          accountId: 1,
          accountName: 'Test',
          accountType: 'account',
          eventType: 'balance_update',
          eventDate: '2026-01-01T00:00:00',
          amountMinor: 0,
          note: null,
          createdAt: '2026-01-01T00:00:00',
          currencyCode: 'EUR',
          currencyMinorUnits: 2,
        },
      ],
      totalCount: 1,
    })
    renderLedgerPage()
    const editBtn = await screen.findByTestId('edit-btn')
    await user.click(editBtn)
    expect(mockSetModalState).toHaveBeenCalledWith(expect.objectContaining({ type: 'editBalanceUpdate' }))
  })

  it('dispatches confirmDeleteEvent modal when delete button is clicked', async () => {
    const user = userEvent.setup()
    renderLedgerPage()
    const deleteBtn = await screen.findByTestId('delete-btn')
    await user.click(deleteBtn)
    expect(mockSetModalState).toHaveBeenCalledWith({ type: 'confirmDeleteEvent', eventId: 42 })
  })
})
