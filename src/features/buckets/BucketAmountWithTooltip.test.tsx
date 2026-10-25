import { describe, it, expect, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import BucketAmountWithTooltip from './BucketAmountWithTooltip'
import type { Currency, SnapshotRow } from '../../shared/types'
import { NO_RATE, converted, makeBucketLink, makeRow, makeTaggedCashflows } from '../../shared/testing/snapshotFixtures'

vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const makeBucket = (overrides?: Partial<SnapshotRow>) =>
  makeRow({ accountId: 10, accountName: 'Holiday', accountType: 'bucket', balanceMinor: 20000, countedMinor: 123000, ...overrides })

const openTooltip = async (row: SnapshotRow) => {
  const user = userEvent.setup()
  render(<BucketAmountWithTooltip row={row} currency={EUR} />)
  await user.hover(screen.getByText('1 230.00 EUR'))
  return within(await screen.findByRole('tooltip'))
}

describe('BucketAmountWithTooltip', () => {
  it('shows the counted total and no tooltip for a bucket without links or tagged cashflows', () => {
    render(<BucketAmountWithTooltip row={makeBucket()} currency={EUR} />)

    expect(screen.getByText('1 230.00 EUR')).toBeInTheDocument()
    expect(screen.queryByText('buckets.balanceBreakdown')).not.toBeInTheDocument()
  })

  it('lists each linked account with its value in the consolidation currency', async () => {
    const tooltip = await openTooltip(
      makeBucket({
        bucketLinks: [
          makeBucketLink({ id: 1, sourceAccountName: 'Savings', valuation: converted(100000) }),
          makeBucketLink({ id: 2, sourceAccountName: 'Pocket money', valuation: converted(2000) }),
        ],
      }),
    )

    expect(tooltip.getByText('Savings').parentElement).toHaveTextContent('1 000.00 EUR')
    expect(tooltip.getByText('Pocket money').parentElement).toHaveTextContent('20.00 EUR')
  })

  it('says a linked account is not counted when it has no rate', async () => {
    const tooltip = await openTooltip(
      makeBucket({
        bucketLinks: [
          makeBucketLink({ id: 1, sourceAccountName: 'Savings', valuation: converted(100000) }),
          makeBucketLink({ id: 2, sourceAccountName: 'Dollars', valuation: NO_RATE }),
        ],
      }),
    )

    expect(tooltip.getByText('Dollars').parentElement).toHaveTextContent('buckets.notCounted')
    expect(tooltip.getByText('Savings').parentElement).not.toHaveTextContent('buckets.notCounted')
  })

  it('lists the tagged cashflows of each source account and skips an empty one', async () => {
    const tooltip = await openTooltip(
      makeBucket({
        taggedCashflows: [
          makeTaggedCashflows({ sourceAccountId: 6, sourceAccountName: 'Revolut', amountMinor: 3000, valuation: converted(2766) }),
          makeTaggedCashflows({ sourceAccountId: 7, sourceAccountName: 'Wise', amountMinor: 0, valuation: converted(0) }),
          makeTaggedCashflows({ sourceAccountId: 8, sourceAccountName: 'Yen card', amountMinor: 900, currencyCode: 'JPY', valuation: NO_RATE }),
        ],
      }),
    )

    expect(tooltip.getByText('buckets.cashflowTaggedBalance: Revolut').parentElement).toHaveTextContent('27.66 EUR')
    expect(tooltip.getByText('buckets.cashflowTaggedBalance: Yen card').parentElement).toHaveTextContent('buckets.notCounted')
    expect(tooltip.queryByText(/Wise/)).not.toBeInTheDocument()
  })

  it('shows the bucket own balance in its own currency and says so when that currency has no rate', async () => {
    const tooltip = await openTooltip(
      makeBucket({
        currencyCode: 'USD',
        balanceMinor: 20000,
        valuation: NO_RATE,
        bucketLinks: [makeBucketLink()],
      }),
    )

    const own = tooltip.getByText('buckets.additionalBalance').parentElement
    expect(own).toHaveTextContent('200.00 USD')
    expect(own).toHaveTextContent('buckets.notCounted')
  })
})
