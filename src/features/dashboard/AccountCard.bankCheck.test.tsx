import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import type { ReactElement, ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import AccountCard from './AccountCard'
import type { SnapshotRow } from '../../shared/types'
import { makeIncomeBudgetRow, makeRow } from '../../shared/testing/snapshotFixtures'
import { confirmBankMatch, deleteBalanceCheck } from '../../shared/api'

vi.mock('react-i18next', async () => {
  const { i18nMock, translate } = await import('../../shared/testing/i18nMock')
  const Trans = ({ i18nKey, values, components }: { i18nKey: string; values?: Record<string, unknown>; components: { amount: ReactElement } }) => (
    <>
      {translate(i18nKey, values)} {components.amount}
    </>
  )
  return { ...i18nMock, Trans }
})

vi.mock('../../shared/api', () => ({
  setBucketEnabled: vi.fn(),
  confirmBankMatch: vi.fn(),
  bookUnexplainedDifference: vi.fn(),
  deleteBalanceCheck: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

vi.mock('../../shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

vi.mock('../buckets/BucketAmountWithTooltip', () => ({
  default: () => <span data-testid="bucket-amount" />,
}))

let queryClient: QueryClient

const Providers = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

const renderCard = (row: SnapshotRow, onFindDifference?: (accountId: number) => void) =>
  render(<AccountCard row={row} onUpdateBalance={() => {}} onDeleteAccount={() => {}} onRenameAccount={() => {}} bankCheck={onFindDifference && { onFindDifference }} />, {
    wrapper: Providers,
  })

const checking = (overrides?: Partial<SnapshotRow>) => makeRow({ accountId: 7, accountName: 'Checking', balanceMinor: 123456, ...overrides })

beforeEach(() => {
  vi.clearAllMocks()
  queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
})

describe('the verification line', () => {
  it('says nothing for an account that was never checked', () => {
    renderCard(checking())

    expect(screen.queryByText(/bankCheck\.verified|bankCheck\.differs/)).not.toBeInTheDocument()
  })

  it('shows the day of a check that still matches', () => {
    renderCard(checking({ verification: { state: 'verified', checkId: 1, day: '2026-11-14' } }))

    expect(screen.getByText('bankCheck.verified (date=14 Nov)')).toBeInTheDocument()
  })

  it('shows the day and how far Saldo now differs, without a sign', () => {
    renderCard(checking({ verification: { state: 'differs', checkId: 1, day: '2026-11-14', differenceMinor: -1250 } }))

    const line = screen.getByText('bankCheck.differs (date=14 Nov)', { exact: false })
    expect(line).toHaveClass('text-amber-700')
    expect(line).toHaveTextContent('1250')
    expect(line).not.toHaveTextContent('-1250')
  })
})

describe('the bank check buttons', () => {
  it('appear on an own account card in live mode', async () => {
    const user = userEvent.setup()
    const onFind = vi.fn()
    renderCard(checking(), onFind)

    expect(screen.getByRole('button', { name: 'bankCheck.matchesBank' })).toHaveAttribute('title', 'bankCheck.matchesBank')
    await user.click(screen.getByRole('button', { name: 'bankCheck.findDifference' }))

    expect(onFind).toHaveBeenCalledWith(7)
  })

  it.each([
    ['a bucket', makeRow({ accountType: 'bucket' })],
    ['an income budget', makeIncomeBudgetRow()],
    ['an asset', makeRow({ accountType: 'asset' })],
  ])('stay off %s', (_kind, row) => {
    renderCard(row, vi.fn())

    expect(screen.queryByRole('button', { name: 'bankCheck.matchesBank' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'bankCheck.findDifference' })).not.toBeInTheDocument()
  })

  it('stay off in time-machine mode, where the dashboard gives no handler', () => {
    renderCard(checking())

    expect(screen.queryByRole('button', { name: 'bankCheck.matchesBank' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'accounts.updateBalance' })).toBeInTheDocument()
  })
})

describe('Matches bank', () => {
  it('records the balance on the card, refreshes, and offers Undo', async () => {
    const user = userEvent.setup()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    ;(confirmBankMatch as Mock).mockResolvedValue(31)
    ;(deleteBalanceCheck as Mock).mockResolvedValue(undefined)
    renderCard(checking(), vi.fn())

    await user.click(screen.getByRole('button', { name: 'bankCheck.matchesBank' }))

    expect(confirmBankMatch).toHaveBeenCalledWith(7, 123456)
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('bankCheck.matched', expect.anything()))
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['snapshot'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['events'] })

    const { action } = (toast.success as Mock).mock.calls[0][1] as { action: { label: string; onClick: () => void } }
    expect(action.label).toBe('bankCheck.undo')
    invalidate.mockClear()
    action.onClick()

    expect(deleteBalanceCheck).toHaveBeenCalledWith(31)
    await waitFor(() => expect(invalidate).toHaveBeenCalledWith({ queryKey: ['snapshot'] }))
  })

  it('says to look again when the balance moved since the card was shown', async () => {
    const user = userEvent.setup()
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
    ;(confirmBankMatch as Mock).mockRejectedValue({ code: 'CHECK_CHANGED', message: 'moved' })
    renderCard(checking(), vi.fn())

    await user.click(screen.getByRole('button', { name: 'bankCheck.matchesBank' }))

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('bankCheck.changed'))
    expect(toast.success).not.toHaveBeenCalled()
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['snapshot'] })
  })

  it('names any other failure', async () => {
    const user = userEvent.setup()
    ;(confirmBankMatch as Mock).mockRejectedValue({ code: 'DB_ERROR', message: 'locked' })
    renderCard(checking(), vi.fn())

    await user.click(screen.getByRole('button', { name: 'bankCheck.matchesBank' }))

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('bankCheck.failed (error=locked)'))
  })
})
