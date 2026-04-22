import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import AccountCard from './AccountCard'
import type { SnapshotRow, Currency } from '../../shared/types'

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

// ── Mock NumberValue — renders its value prop as text ────────────────────────
vi.mock('../../shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

// ── Mock BucketAmountWithTooltip — renders a placeholder ─────────────────────
vi.mock('../buckets/BucketAmountWithTooltip', () => ({
  default: () => <span data-testid="bucket-amount">bucket-amount</span>,
}))

// ── Helpers ──────────────────────────────────────────────────────────────────
const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const makeSnapshot = (overrides?: Partial<SnapshotRow>): SnapshotRow => ({
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  iban: null,
  balanceMinor: 100000,
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
  isCustom: false,
  convertedBalanceMinor: 100000,
  fxRateMissing: false,
  isLinkedToAsset: false,
  linkedAssetIds: [],
  isBucketLinked: false,
  bucketLinks: [],
  linkedBalanceMinor: 0,
  cashflowTaggedMinor: 0,
  personId: null,
  purchasePriceMinor: null,
  purchaseDate: null,
  depreciationPeriodMonths: null,
  ...overrides,
})

// ── Test cases ───────────────────────────────────────────────────────────────

describe('AccountCard', () => {
  it('renders bucket-type row without IBAN section, shows bucket balance', () => {
    const row = makeSnapshot({
      accountId: 10,
      accountName: 'Vacation Fund',
      accountType: 'bucket',
      balanceMinor: 50000,
      convertedBalanceMinor: 50000,
    })

    render(<AccountCard row={row} consolidationCurrency={EUR} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

    expect(screen.getByText('Vacation Fund')).toBeInTheDocument()
    expect(screen.getByTestId('bucket-amount')).toBeInTheDocument()
    expect(screen.queryByText(/SK/)).not.toBeInTheDocument()
  })

  it('renders asset-type row with isCustom: true and consolidation currency — shows converted balance', () => {
    const row = makeSnapshot({
      accountId: 20,
      accountName: 'Gold Vault',
      accountType: 'asset',
      isCustom: true,
      balanceMinor: 30000000,
      currencyCode: 'XAU',
      currencyMinorUnits: 0,
      convertedBalanceMinor: 45000000,
    })

    render(<AccountCard row={row} consolidationCurrency={EUR} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

    expect(screen.getByText('Gold Vault')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('45000000')
  })

  it('renders standard account with foreign currency different from consolidation currency', () => {
    const row = makeSnapshot({
      accountId: 30,
      accountName: 'USD Account',
      currencyCode: 'USD',
      currencyMinorUnits: 2,
      balanceMinor: 120000,
      convertedBalanceMinor: 110000,
    })

    render(<AccountCard row={row} consolidationCurrency={EUR} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

    expect(screen.getByText('USD Account')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('120000')
  })

  it('renders standard account in same currency as consolidation currency', () => {
    const row = makeSnapshot({
      accountId: 40,
      accountName: 'Main EUR',
      currencyCode: 'EUR',
      balanceMinor: 250000,
      convertedBalanceMinor: 250000,
    })

    render(<AccountCard row={row} consolidationCurrency={EUR} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

    expect(screen.getByText('Main EUR')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('250000')
  })

  it('renders equity tooltip when asset has linkedAssetIds and allAccounts provided', () => {
    const row = makeSnapshot({
      accountId: 50,
      accountName: 'Real Estate',
      accountType: 'asset',
      balanceMinor: 10000000,
      convertedBalanceMinor: 10000000,
      linkedAssetIds: [60, 70],
    })
    const linkedAccounts: SnapshotRow[] = [
      makeSnapshot({ accountId: 60, accountName: 'Mortgage A', convertedBalanceMinor: 4000000 }),
      makeSnapshot({ accountId: 70, accountName: 'Mortgage B', convertedBalanceMinor: 3000000 }),
    ]

    render(<AccountCard row={row} consolidationCurrency={EUR} allAccounts={linkedAccounts} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

    expect(screen.getByText('Real Estate')).toBeInTheDocument()
    const equityValues = screen.getAllByTestId('number-value')
    expect(equityValues).toHaveLength(2)
    expect(equityValues[1]).toHaveTextContent('17000000') // 10M + 4M + 3M
  })

  it('renders IBAN segments for a standard account with an IBAN value', () => {
    const row = makeSnapshot({
      accountId: 80,
      accountName: 'IBAN Account',
      iban: 'SK1234567890',
      balanceMinor: 75000,
      convertedBalanceMinor: 75000,
    })

    render(<AccountCard row={row} consolidationCurrency={EUR} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

    expect(screen.getByText('IBAN Account')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('75000')
    expect(screen.getByText('SK12 3456 7890')).toBeInTheDocument()
  })
})
