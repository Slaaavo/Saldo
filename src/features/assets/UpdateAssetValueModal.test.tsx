import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import UpdateAssetValueModal from './UpdateAssetValueModal'
import type { Currency } from '../../shared/types'

// ── Mock the API layer ──────────────────────────────────────────────────────
vi.mock('../../shared/api', () => ({
  listFxRates: vi.fn(),
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

// ── Mock DatePicker to avoid Popover/Calendar complexity in tests ────────────
vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

// ── Prevent sonner toast from emitting warnings in the test environment ──────
vi.mock('sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}))

import { toast } from 'sonner'
import { listFxRates } from '../../shared/api'

// ── Helpers ─────────────────────────────────────────────────────────────────

const EUR: Currency = {
  id: 1,
  code: 'EUR',
  name: 'Euro',
  minorUnits: 2,
  isCustom: false,
}

const setupMocks = () => {
  ;(listFxRates as Mock).mockResolvedValue([
    {
      id: 1,
      date: '2026-01-02',
      fromCurrencyCode: 'EUR',
      toCurrencyCode: 'GOLD',
      rateMantissa: 1,
      rateExponent: -3,
      isManual: false,
      fetchedAt: '2026-01-02T12:00:00',
      isDirect: true,
    },
  ])
}

const defaultProps = {
  accountId: 42,
  accountName: 'Gold Reserve',
  currencyCode: 'GOLD',
  currencyMinorUnits: 2,
  balanceMinor: 200, // 2.00 units
  consolidationCurrency: EUR,
  onSubmit: vi.fn(),
  onClose: vi.fn(),
}

// ── Tests ───────────────────────────────────────────────────────────────────
describe('UpdateAssetValueModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('loads price with full precision (not truncated to 2 decimal places)', async () => {
    setupMocks()
    render(<UpdateAssetValueModal {...defaultProps} />)

    await waitFor(() => {
      expect(screen.getByDisplayValue('0.001')).toBeInTheDocument()
    })
  })

  it('submits null for pricePerUnit when price and date are unchanged', async () => {
    setupMocks()
    const onSubmit = vi.fn()
    const user = userEvent.setup()
    render(<UpdateAssetValueModal {...defaultProps} onSubmit={onSubmit} />)

    // Wait for the rate-derived price to populate the input
    await waitFor(() => {
      expect(screen.getByDisplayValue('0.001')).toBeInTheDocument()
    })

    // The date defaults to today; the price is only "unchanged" on the date of the rate it came from
    const datePicker = screen.getByTestId('date-picker')
    await user.clear(datePicker)
    await user.type(datePicker, '2026-01-02')
    await user.click(screen.getByText('modals.editBalanceUpdate.submit'))

    expect(onSubmit).toHaveBeenCalledWith(
      42, // accountId
      null, // amountMinor — quantity unchanged
      null, // pricePerUnit — price unchanged
      '2026-01-02', // date
      null, // note
    )
  })

  it('submits the prefilled price when only the date is changed', async () => {
    setupMocks()
    const onSubmit = vi.fn()
    const user = userEvent.setup()
    render(<UpdateAssetValueModal {...defaultProps} onSubmit={onSubmit} />)

    await waitFor(() => {
      expect(screen.getByDisplayValue('0.001')).toBeInTheDocument()
    })

    const datePicker = screen.getByTestId('date-picker')
    await user.clear(datePicker)
    await user.type(datePicker, '2026-02-15')
    await user.click(screen.getByText('modals.editBalanceUpdate.submit'))

    expect(onSubmit).toHaveBeenCalledWith(42, null, '0.001', '2026-02-15', null)
  })

  it('ignores prices quoted against a different consolidation currency', async () => {
    ;(listFxRates as Mock).mockResolvedValue([
      {
        id: 1,
        date: '2026-01-02',
        fromCurrencyCode: 'USD',
        toCurrencyCode: 'GOLD',
        rateMantissa: 5,
        rateExponent: -1,
        isManual: false,
        fetchedAt: '2026-01-02T12:00:00',
        isDirect: true,
      },
    ])
    render(<UpdateAssetValueModal {...defaultProps} />)

    await waitFor(() => expect(listFxRates).toHaveBeenCalled())

    expect(screen.queryByDisplayValue('0.5')).not.toBeInTheDocument()
  })

  it('shows a toast when the prices cannot be loaded', async () => {
    ;(listFxRates as Mock).mockRejectedValue({ code: 'DB', message: 'offline' })
    render(<UpdateAssetValueModal {...defaultProps} />)

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('offline'))
  })

  it('submits full-precision price when price is changed', async () => {
    setupMocks()
    const onSubmit = vi.fn()
    const user = userEvent.setup()
    render(<UpdateAssetValueModal {...defaultProps} onSubmit={onSubmit} />)

    const priceInput = await waitFor(() => screen.getByDisplayValue('0.001'))

    await user.clear(priceInput)
    await user.type(priceInput, '0.005')

    await user.click(screen.getByText('modals.editBalanceUpdate.submit'))

    expect(onSubmit).toHaveBeenCalledWith(
      42, // accountId
      null, // amountMinor — quantity unchanged
      '0.005', // pricePerUnit — changed, submitted as decimal string
      expect.any(String), // date
      null, // note
    )
  })
})
