import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import AccountCard from './AccountCard'
import type { SnapshotRow, Currency } from '../../shared/types'
import { NO_RATE, converted, makeRow } from '../../shared/testing/snapshotFixtures'

// ── Mock i18n — pass-through that returns the key, with interpolation values appended ─────
vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

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

const renderCard = (row: SnapshotRow, allAccounts?: SnapshotRow[]) =>
  render(<AccountCard row={row} consolidationCurrency={EUR} allAccounts={allAccounts} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} />)

// ── Test cases ───────────────────────────────────────────────────────────────

describe('AccountCard', () => {
  it('renders bucket-type row without IBAN section, shows bucket balance', () => {
    renderCard(makeRow({ accountId: 10, accountName: 'Vacation Fund', accountType: 'bucket', balanceMinor: 50000 }))

    expect(screen.getByText('Vacation Fund')).toBeInTheDocument()
    expect(screen.getByTestId('bucket-amount')).toBeInTheDocument()
    expect(screen.queryByText(/SK/)).not.toBeInTheDocument()
  })

  it('renders asset-type row with isCustom: true and consolidation currency — shows converted balance', () => {
    renderCard(
      makeRow({
        accountId: 20,
        accountName: 'Gold Vault',
        accountType: 'asset',
        isCustom: true,
        balanceMinor: 30000000,
        currencyCode: 'XAU',
        currencyMinorUnits: 0,
        valuation: converted(45000000, { isDirect: true }),
        countedMinor: 45000000,
      }),
    )

    expect(screen.getByText('Gold Vault')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('45000000')
  })

  it('renders standard account with foreign currency different from consolidation currency', () => {
    renderCard(
      makeRow({
        accountId: 30,
        accountName: 'USD Account',
        currencyCode: 'USD',
        balanceMinor: 120000,
        valuation: converted(110000),
        countedMinor: 110000,
      }),
    )

    expect(screen.getByText('USD Account')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('120000')
  })

  it('renders standard account in same currency as consolidation currency', () => {
    renderCard(makeRow({ accountId: 40, accountName: 'Main EUR', currencyCode: 'EUR', balanceMinor: 250000 }))

    expect(screen.getByText('Main EUR')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('250000')
  })

  it('shows the native amount and says so when the account has no rate', () => {
    renderCard(makeRow({ accountId: 31, accountName: 'USD Account', currencyCode: 'USD', balanceMinor: 120000, valuation: NO_RATE, countedMinor: 0 }))

    expect(screen.getByTestId('number-value')).toHaveTextContent('120000')
    expect(screen.getByText('accounts.noRateTooltip')).toBeInTheDocument()
  })

  it('does not mention a missing rate for an account that has one', () => {
    renderCard(makeRow({ accountId: 30, currencyCode: 'USD', balanceMinor: 120000, valuation: converted(110000), countedMinor: 110000 }))

    expect(screen.queryByText('accounts.noRateTooltip')).not.toBeInTheDocument()
  })

  it('adds the rate date and age to the hover note when the rate is old', async () => {
    const user = userEvent.setup()
    renderCard(
      makeRow({
        accountId: 30,
        currencyCode: 'USD',
        balanceMinor: 120000,
        valuation: converted(110000, { date: '2026-10-12', ageDays: 13, stale: true }),
        countedMinor: 110000,
      }),
    )

    await user.hover(screen.getByTestId('number-value'))

    expect((await screen.findAllByText('accounts.staleRateTooltip (date=12 October 2026, count=13)')).length).toBeGreaterThan(0)
  })

  it('shows the equity Rust computed, not a sum of the linked rows', () => {
    const row = makeRow({
      accountId: 50,
      accountName: 'Real Estate',
      accountType: 'asset',
      balanceMinor: 10000000,
      linkedAssetIds: [60, 70],
      equityMinor: 12345678,
    })
    const linkedAccounts = [
      makeRow({ accountId: 60, accountName: 'Mortgage A', balanceMinor: -4000000 }),
      makeRow({ accountId: 70, accountName: 'Mortgage B', balanceMinor: -3000000 }),
    ]

    renderCard(row, linkedAccounts)

    expect(screen.getByText('Real Estate')).toBeInTheDocument()
    const values = screen.getAllByTestId('number-value')
    expect(values).toHaveLength(2)
    expect(values[1]).toHaveTextContent('12345678')
  })

  it('hides equity while a part has no rate', () => {
    const row = makeRow({ accountId: 50, accountName: 'Real Estate', accountType: 'asset', balanceMinor: 10000000, linkedAssetIds: [60], equityMinor: null })

    renderCard(row, [makeRow({ accountId: 60, accountName: 'Mortgage A' })])

    expect(screen.getAllByTestId('number-value')).toHaveLength(1)
    expect(screen.queryByText('assets.equity', { exact: false })).not.toBeInTheDocument()
  })

  it('renders IBAN segments for a standard account with an IBAN value', () => {
    renderCard(makeRow({ accountId: 80, accountName: 'IBAN Account', iban: 'SK1234567890', balanceMinor: 75000 }))

    expect(screen.getByText('IBAN Account')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('75000')
    // Each 4-char group is its own <span>, so match segments and check the joined line
    expect(screen.getByText('SK12')).toBeInTheDocument()
    expect(screen.getByText('3456')).toBeInTheDocument()
    expect(screen.getByText('7890').closest('p')).toHaveTextContent('SK12 3456 7890')
  })
})
