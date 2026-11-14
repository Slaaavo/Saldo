import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import type { ReactElement, ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import FindBankDifferenceModal from './FindBankDifferenceModal'
import { makeRow } from '@/shared/testing/snapshotFixtures'
import { bookUnexplainedDifference, confirmBankMatch } from '@/shared/api'

vi.mock('react-i18next', async () => {
  const { i18nMock, translate } = await import('@/shared/testing/i18nMock')
  const Trans = ({ i18nKey, values, components }: { i18nKey: string; values?: Record<string, unknown>; components: { amount: ReactElement } }) => (
    <>
      {translate(i18nKey, values)} {components.amount}
    </>
  )
  return { ...i18nMock, Trans }
})

vi.mock('@/shared/api', () => ({
  confirmBankMatch: vi.fn(),
  bookUnexplainedDifference: vi.fn(),
  deleteBalanceCheck: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

vi.mock('@/shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

let queryClient: QueryClient

const Providers = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

const checking = makeRow({ accountId: 7, accountName: 'Checking', balanceMinor: 100000 })

const renderModal = (onClose = vi.fn()) => {
  render(<FindBankDifferenceModal row={checking} onClose={onClose} />, { wrapper: Providers })
  return onClose
}

const typeBankBalance = async (value: string) => {
  const user = userEvent.setup()
  await user.type(screen.getByLabelText('bankCheck.dialog.bankBalance'), value)
  return user
}

const difference = () => screen.getByText(/bankCheck\.dialog\.(lower|higher|same)/)

beforeEach(() => {
  vi.clearAllMocks()
  queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
})

describe('FindBankDifferenceModal', () => {
  it("shows Saldo's balance and asks for the bank's before it offers anything", () => {
    renderModal()

    expect(screen.getByText('bankCheck.dialog.title (name=Checking)')).toBeInTheDocument()
    expect(screen.getByTestId('number-value')).toHaveTextContent('100000')
    expect(screen.queryByText(/bankCheck\.dialog\.(lower|higher|same)/)).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'bankCheck.dialog.book' })).toBeDisabled()
  })

  it('says Saldo is lower when the bank shows more, and books the difference', async () => {
    const onClose = renderModal()
    ;(bookUnexplainedDifference as Mock).mockResolvedValue({ eventId: 5, checkId: 6 })

    const user = await typeBankBalance('1012.50')

    expect(difference()).toHaveTextContent('bankCheck.dialog.lower 1250')
    expect(screen.queryByRole('button', { name: 'bankCheck.dialog.markAsMatching' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'bankCheck.dialog.book' }))

    expect(bookUnexplainedDifference).toHaveBeenCalledWith(7, 101250, 100000)
    await waitFor(() => expect(onClose).toHaveBeenCalled())
    expect(toast.success).toHaveBeenCalledWith('bankCheck.dialog.booked')
  })

  it('says Saldo is higher when the bank shows less, without a sign', async () => {
    renderModal()

    await typeBankBalance('987.50')

    expect(difference()).toHaveTextContent('bankCheck.dialog.higher 1250')
    expect(difference()).not.toHaveTextContent('-')
  })

  it('offers Mark as matching when there is no difference', async () => {
    const onClose = renderModal()
    ;(confirmBankMatch as Mock).mockResolvedValue(9)

    const user = await typeBankBalance('1000')

    expect(difference()).toHaveTextContent('bankCheck.dialog.same')
    expect(screen.queryByRole('button', { name: 'bankCheck.dialog.book' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'bankCheck.dialog.markAsMatching' }))

    expect(confirmBankMatch).toHaveBeenCalledWith(7, 100000)
    expect(bookUnexplainedDifference).not.toHaveBeenCalled()
    await waitFor(() => expect(onClose).toHaveBeenCalled())
    expect(toast.success).toHaveBeenCalledWith('bankCheck.matched', expect.anything())
  })

  it('stays open and says to look again when the balance moved', async () => {
    const onClose = renderModal()
    ;(bookUnexplainedDifference as Mock).mockRejectedValue({ code: 'CHECK_CHANGED', message: 'moved' })

    const user = await typeBankBalance('1012.50')
    await user.click(screen.getByRole('button', { name: 'bankCheck.dialog.book' }))

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('bankCheck.changed'))
    expect(onClose).not.toHaveBeenCalled()
  })

  it('closes on Cancel without writing', async () => {
    const onClose = renderModal()
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: 'bankCheck.dialog.cancel' }))

    expect(onClose).toHaveBeenCalled()
    expect(bookUnexplainedDifference).not.toHaveBeenCalled()
    expect(confirmBankMatch).not.toHaveBeenCalled()
  })
})
