import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import PortfolioItemFilter from './PortfolioItemFilter'
import { makeIncomeBudgetRow, makeRow } from '../../shared/testing/snapshotFixtures'

vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

describe('PortfolioItemFilter', () => {
  it('lists every account and bucket except an income budget, which has no entries to filter by', async () => {
    const user = userEvent.setup()
    render(
      <PortfolioItemFilter
        accounts={[makeRow({ accountId: 1, accountName: 'Checking' }), makeRow({ accountId: 10, accountName: 'Vacation', accountType: 'bucket' }), makeIncomeBudgetRow()]}
        selectedIds={[]}
        onChange={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'ledgerPage.portfolioItem' }))

    expect(await screen.findByRole('button', { name: 'Vacation' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Checking' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Consulting income' })).not.toBeInTheDocument()
  })

  it('names the account when one is selected and counts them when several are', () => {
    const accounts = [makeRow({ accountId: 1, accountName: 'Checking' }), makeRow({ accountId: 2, accountName: 'Savings' })]
    const { rerender } = render(<PortfolioItemFilter accounts={accounts} selectedIds={[2]} onChange={vi.fn()} />)

    expect(screen.getByRole('button', { name: 'Savings' })).toBeInTheDocument()

    rerender(<PortfolioItemFilter accounts={accounts} selectedIds={[1, 2]} onChange={vi.fn()} />)

    expect(screen.getByRole('button', { name: 'ledgerPage.portfolioItemCount (count=2)' })).toBeInTheDocument()
  })
})
