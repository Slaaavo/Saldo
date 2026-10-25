import { describe, it, expect, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import MetricBreakdownDialog from './MetricBreakdownDialog'
import type { Currency, Metric } from '../../shared/types'
import { makeCountedLine, makeMetric, makeMoney, makeRate, makeSkippedLine } from '../../shared/testing/snapshotFixtures'

vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const renderDialog = (metric: Metric, onClose = vi.fn()) => render(<MetricBreakdownDialog metric={metric} currency={EUR} onClose={onClose} />)

const rowOf = (accountName: string) => screen.getByRole('row', { name: new RegExp(accountName) })

const checking = makeCountedLine({ accountId: 1, accountName: 'Checking', native: makeMoney(100000), signedMinor: 100000 })
const dollars = makeCountedLine({
  accountId: 2,
  accountName: 'Dollar account',
  native: makeMoney(200000, 'USD'),
  rate: makeRate({ mantissa: 10842, exponent: -4, date: '2026-10-20' }),
  signedMinor: 184468,
})

describe('MetricBreakdownDialog', () => {
  it('titles the dialog with the metric label and its total', () => {
    renderDialog(makeMetric('netWorth', { counted: [checking, dollars] }))

    const dialog = screen.getByRole('dialog', { name: /metrics\.netWorth/ })
    expect(within(dialog).getByRole('heading', { name: /metrics\.netWorth/ })).toHaveTextContent('2 844.68 EUR')
  })

  it('shows a counted line with its native amount, rate quote, rate date and signed value', () => {
    renderDialog(makeMetric('netWorth', { counted: [checking, dollars] }))

    const row = within(rowOf('Dollar account'))
    expect(row.getByText('2 000.00 USD')).toBeInTheDocument()
    expect(row.getByText('1 EUR = 1.0842 USD')).toBeInTheDocument()
    expect(row.getByText('20 October 2026')).toBeInTheDocument()
    expect(row.getAllByRole('cell')[3]).toHaveTextContent(/^\+1 844\.68 EUR$/)
  })

  it('says there is no conversion for an amount already in the consolidation currency', () => {
    renderDialog(makeMetric('netWorth', { counted: [checking] }))

    const row = within(rowOf('Checking'))
    expect(row.getByText('metrics.breakdown.noConversion')).toBeInTheDocument()
    expect(row.getAllByRole('cell')[3]).toHaveTextContent(/^\+1 000\.00 EUR$/)
  })

  it('shows the age of an old rate and marks a manual rate', () => {
    const pounds = makeCountedLine({
      accountId: 3,
      accountName: 'Pound account',
      native: makeMoney(80000, 'GBP'),
      rate: makeRate({ mantissa: 8, exponent: -1, date: '2026-10-12', isManual: true, ageDays: 13, stale: true }),
      signedMinor: 100000,
    })
    renderDialog(makeMetric('netWorth', { counted: [dollars, pounds] }))

    const old = within(rowOf('Pound account'))
    expect(old.getByText('metrics.breakdown.rateAge (count=13)')).toBeInTheDocument()
    expect(old.getByText('metrics.breakdown.manual')).toBeInTheDocument()
    const fresh = within(rowOf('Dollar account'))
    expect(fresh.queryByText(/metrics\.breakdown\.rateAge/)).not.toBeInTheDocument()
    expect(fresh.queryByText('metrics.breakdown.manual')).not.toBeInTheDocument()
  })

  it('quotes a custom unit as its price in the consolidation currency', () => {
    const unit = makeCountedLine({
      accountId: 4,
      accountName: 'Crypto',
      accountType: 'asset',
      native: makeMoney(500, 'VBTC', 2),
      rate: makeRate({ mantissa: 2340, exponent: -2, isDirect: true }),
      signedMinor: 11700,
    })
    renderDialog(makeMetric('netWorth', { counted: [unit] }))

    expect(within(rowOf('Crypto')).getByText('1 VBTC = 23.4 EUR')).toBeInTheDocument()
  })

  it('tells the parts of a bucket apart and shows what is subtracted with its sign', () => {
    const part = { accountId: 10, accountName: 'Holiday', accountType: 'bucket' as const }
    renderDialog(
      makeMetric('leftToSpend', {
        counted: [
          makeCountedLine({ ...part, part: { kind: 'balance' }, native: makeMoney(20000), signedMinor: -20000 }),
          makeCountedLine({ ...part, part: { kind: 'linkedAccount', sourceAccountId: 5, sourceAccountName: 'Savings' }, native: makeMoney(100000), signedMinor: -100000 }),
          makeCountedLine({ ...part, part: { kind: 'taggedCashflows', sourceAccountId: 6, sourceAccountName: 'Revolut' }, native: makeMoney(3000), signedMinor: -3000 }),
        ],
      }),
    )

    const rows = screen.getAllByRole('row', { name: /Holiday/ })
    expect(rows).toHaveLength(3)
    expect(within(rows[0]).getByText('buckets.additionalBalance')).toBeInTheDocument()
    expect(within(rows[1]).getByText('metrics.breakdown.part.linkedAccount (account=Savings)')).toBeInTheDocument()
    expect(within(rows[2]).getByText('metrics.breakdown.part.taggedCashflows (account=Revolut)')).toBeInTheDocument()
    expect(within(rows[1]).getAllByRole('cell')[3]).toHaveTextContent(/^-1 000\.00 EUR$/)
  })

  it('shows the total Rust returned in the total row, even when it differs from the lines', () => {
    renderDialog(makeMetric('totalBalance', { counted: [checking, dollars], totalMinor: 99900 }))

    expect(screen.getByRole('row', { name: /metrics\.breakdown\.total/ })).toHaveTextContent('999.00 EUR')
  })

  it('lists what is not counted with its amount and the reason', () => {
    renderDialog(
      makeMetric('liquid', {
        counted: [checking],
        skipped: [
          makeSkippedLine({ accountId: 7, accountName: 'Yen account', amount: makeMoney(900000, 'JPY', 0), reason: 'noRate' }),
          makeSkippedLine({ accountId: 8, accountName: 'Mortgage', amount: makeMoney(-5000000), reason: 'linkedToAsset' }),
          makeSkippedLine({ accountId: 9, accountName: 'House', accountType: 'asset', amount: makeMoney(30000000), reason: 'notLiquid' }),
          makeSkippedLine({ accountId: 10, accountName: 'Holiday', accountType: 'bucket', part: { kind: 'bucketTotal' }, amount: makeMoney(123000), reason: 'earmark' }),
        ],
      }),
    )

    const notCounted = within(screen.getByRole('list', { name: 'metrics.breakdown.notCounted' }))
    const yen = notCounted.getByText('Yen account').closest('li')!
    expect(yen).toHaveTextContent('900 000 JPY')
    expect(yen).toHaveTextContent('metrics.breakdown.reason.noRate (currency=JPY)')
    expect(notCounted.getByText('Mortgage').closest('li')).toHaveTextContent('-50 000.00 EUR')
    expect(notCounted.getByText('Mortgage').closest('li')).toHaveTextContent('metrics.breakdown.reason.linkedToAsset')
    expect(notCounted.getByText('House').closest('li')).toHaveTextContent('metrics.breakdown.reason.notLiquid')
    const bucket = notCounted.getByText('Holiday').closest('li')!
    expect(bucket).toHaveTextContent('metrics.breakdown.reason.earmark')
    expect(bucket).toHaveTextContent('metrics.breakdown.part.bucketTotal')
    expect(bucket).toHaveTextContent('1 230.00 EUR')
  })

  it('leaves out the not-counted list when every row is counted', () => {
    renderDialog(makeMetric('totalBalance', { counted: [checking] }))

    expect(screen.queryByText('metrics.breakdown.notCounted')).not.toBeInTheDocument()
  })

  it('says so when nothing is counted', () => {
    renderDialog(makeMetric('totalBalance', { counted: [] }))

    expect(screen.getByText('metrics.breakdown.nothingCounted')).toBeInTheDocument()
    expect(screen.getByRole('row', { name: /metrics\.breakdown\.total/ })).toHaveTextContent('0.00 EUR')
  })

  it('closes with Escape', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()
    renderDialog(makeMetric('totalBalance', { counted: [checking] }), onClose)

    await user.keyboard('{Escape}')

    expect(onClose).toHaveBeenCalledOnce()
  })
})
