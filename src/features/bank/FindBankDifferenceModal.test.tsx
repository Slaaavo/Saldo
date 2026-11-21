import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { Fragment, type ReactElement, type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import FindBankDifferenceModal from './FindBankDifferenceModal'
import { makeRow } from '@/shared/testing/snapshotFixtures'
import { bookUnexplainedDifference, confirmBankMatch, deleteEvent, deleteSplitGroup, findDifferenceCauses, updateEvent } from '@/shared/api'
import type { BankLine, DifferenceCause, RowTarget } from '@/shared/types'

vi.mock('react-i18next', async () => {
  const { i18nMock, translate } = await import('@/shared/testing/i18nMock')
  const Trans = ({ i18nKey, values, components }: { i18nKey: string; values?: Record<string, unknown>; components: Record<string, ReactElement> }) => (
    <>
      {translate(i18nKey, values)}{' '}
      {Object.entries(components).map(([name, component]) => (
        <Fragment key={name}>{component}</Fragment>
      ))}
    </>
  )
  return { ...i18nMock, Trans }
})

vi.mock('@/shared/api', () => ({
  confirmBankMatch: vi.fn(),
  bookUnexplainedDifference: vi.fn(),
  deleteBalanceCheck: vi.fn(),
  findDifferenceCauses: vi.fn(),
  deleteEvent: vi.fn(),
  deleteSplitGroup: vi.fn(),
  updateEvent: vi.fn(),
}))

const mockSetModalState = vi.fn()
vi.mock('@/app/useModal', () => ({
  useModal: () => ({ setModalState: mockSetModalState, closeModal: vi.fn(), modalState: { type: 'none' } }),
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
  ;(findDifferenceCauses as Mock).mockResolvedValue([])
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

const bankLine = <Target extends RowTarget>(
  target: Target,
  amountMinor: number,
  eventDate: string,
  labels: Partial<Pick<BankLine, 'note' | 'counterpartName'>> = {},
): BankLine<Target> => ({
  target,
  eventDate,
  amountMinor,
  note: null,
  counterpartName: null,
  ...labels,
})

const copy = bankLine({ kind: 'cashflow', eventId: 42 }, -1250, '2026-11-12T12:00:00', { note: 'Lidl' })
const original = bankLine({ kind: 'cashflow', eventId: 41 }, -1250, '2026-11-10T12:00:00', { note: 'Lidl' })
const wrongSign = bankLine({ kind: 'cashflow', eventId: 43 }, -625, '2026-11-13T09:30:00', { note: 'Refund' })

const listCauses = (causes: DifferenceCause[]) => (findDifferenceCauses as Mock).mockResolvedValue(causes)

const confirmDialog = (title: string) => screen.getByRole('dialog', { name: title })

describe('FindBankDifferenceModal causes', () => {
  it('lists the likely causes for the typed balance, each with its fix', async () => {
    listCauses([
      { kind: 'nearCopy', row: copy, copyOf: original },
      { kind: 'flippedSign', row: wrongSign },
    ])
    renderModal()

    await typeBankBalance('1012.50')

    expect(await screen.findByText('bankCheck.causes.title')).toBeInTheDocument()
    expect(findDifferenceCauses).toHaveBeenCalledWith(7, 101250, 100000)
    const [nearCopy, flippedSign] = screen.getAllByRole('listitem')
    expect(nearCopy).toHaveTextContent('bankCheck.causes.nearCopy (day=10 Nov)12 Nov · Lidl · -1250bankCheck.causes.deleteCopy')
    expect(flippedSign).toHaveTextContent('bankCheck.causes.flippedSign13 Nov · Refund · -625bankCheck.causes.flipSign')
    expect(screen.queryByText('bankCheck.causes.none')).not.toBeInTheDocument()
  })

  it('deletes a copy after a confirm and stays open with the typed balance', async () => {
    listCauses([{ kind: 'nearCopy', row: copy, copyOf: original }])
    ;(deleteEvent as Mock).mockResolvedValue(undefined)
    const onClose = renderModal()

    const user = await typeBankBalance('1012.50')
    await user.click(await screen.findByRole('button', { name: 'bankCheck.causes.deleteCopy' }))

    expect(deleteEvent).not.toHaveBeenCalled()
    expect(confirmDialog('modals.confirm.deleteTitle')).toHaveTextContent('bankCheck.causes.confirm.deleteRow 12 Nov · Lidl-1250')
    await user.click(screen.getByRole('button', { name: 'common.delete' }))

    await waitFor(() => expect(deleteEvent).toHaveBeenCalledWith(42))
    expect(toast.success).toHaveBeenCalledWith('bankCheck.causes.deleted')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'modals.confirm.deleteTitle' })).not.toBeInTheDocument())
    expect(onClose).not.toHaveBeenCalled()
    expect(screen.getByLabelText('bankCheck.dialog.bankBalance')).toHaveValue(1012.5)
  })

  it('lists a booked difference the real row explains first and deletes it after a confirm, staying open', async () => {
    const booked = bankLine({ kind: 'unexplainedDifference', eventId: 60 }, -1250, '2026-11-14T10:00:00')
    const real = bankLine({ kind: 'cashflow', eventId: 61 }, -1250, '2026-11-13T12:00:00', { note: 'Card' })
    listCauses([
      { kind: 'bookedDifferenceExplained', row: booked },
      { kind: 'equalsGap', row: real },
    ])
    ;(deleteEvent as Mock).mockResolvedValue(undefined)
    const onClose = renderModal()

    const user = await typeBankBalance('1012.50')
    await screen.findByText('bankCheck.causes.title')
    const [explained, equalsGap] = screen.getAllByRole('listitem')
    expect(explained).toHaveTextContent('bankCheck.causes.bookedDifferenceExplained (day=14 Nov)14 Nov · -1250bankCheck.causes.deleteBookedDifference')
    expect(equalsGap).toHaveTextContent('bankCheck.causes.equalsGap13 Nov · Card · -1250bankCheck.causes.deleteRow')
    await user.click(screen.getByRole('button', { name: 'bankCheck.causes.deleteBookedDifference' }))

    expect(confirmDialog('modals.confirm.deleteTitle')).toHaveTextContent('bankCheck.causes.confirm.deleteBookedDifference 14 Nov-1250')
    await user.click(screen.getByRole('button', { name: 'common.delete' }))

    await waitFor(() => expect(deleteEvent).toHaveBeenCalledWith(60))
    expect(toast.success).toHaveBeenCalledWith('bankCheck.causes.deleted')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'modals.confirm.deleteTitle' })).not.toBeInTheDocument())
    expect(onClose).not.toHaveBeenCalled()
    expect(screen.getByLabelText('bankCheck.dialog.bankBalance')).toHaveValue(1012.5)
  })

  it('flips a sign with the same date and note after a confirm', async () => {
    listCauses([{ kind: 'flippedSign', row: wrongSign }])
    ;(updateEvent as Mock).mockResolvedValue(undefined)
    renderModal()

    const user = await typeBankBalance('1012.50')
    await user.click(await screen.findByRole('button', { name: 'bankCheck.causes.flipSign' }))

    expect(confirmDialog('modals.confirm.title')).toHaveTextContent('bankCheck.causes.confirm.flip 13 Nov · Refund-625625')
    await user.click(screen.getByRole('button', { name: 'modals.confirm.submit' }))

    await waitFor(() => expect(updateEvent).toHaveBeenCalledWith(43, 625, '2026-11-13T09:30:00', 'Refund'))
    expect(toast.success).toHaveBeenCalledWith('bankCheck.causes.flipped')
  })

  it('says a transfer leaves both accounts and a split group goes with all its parts', async () => {
    const transfer = bankLine({ kind: 'transfer', eventId: 50 }, -1250, '2026-11-12T08:00:00', { counterpartName: 'Savings' })
    const split = bankLine({ kind: 'splitGroup', splitGroupId: 9, parts: 3 }, -1250, '2026-11-11T08:00:00', { note: 'Groceries' })
    listCauses([
      { kind: 'equalsGap', row: transfer },
      { kind: 'equalsGap', row: split },
    ])
    ;(deleteEvent as Mock).mockResolvedValue(undefined)
    ;(deleteSplitGroup as Mock).mockResolvedValue(undefined)
    renderModal()

    const user = await typeBankBalance('1012.50')
    await screen.findByText('bankCheck.causes.title')
    const [deleteTransfer, deleteSplit] = screen.getAllByRole('button', { name: 'bankCheck.causes.deleteRow' })

    await user.click(deleteTransfer)
    expect(confirmDialog('modals.confirm.deleteTitle')).toHaveTextContent('bankCheck.causes.confirm.deleteTransfer 12 Nov · Savings-1250')
    await user.click(screen.getByRole('button', { name: 'common.delete' }))
    await waitFor(() => expect(deleteEvent).toHaveBeenCalledWith(50))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'modals.confirm.deleteTitle' })).not.toBeInTheDocument())

    await user.click(deleteSplit)
    expect(confirmDialog('modals.confirm.deleteTitle')).toHaveTextContent('bankCheck.causes.confirm.deleteSplitGroup (parts=3) 11 Nov · Groceries-1250')
    await user.click(screen.getByRole('button', { name: 'common.delete' }))
    await waitFor(() => expect(deleteSplitGroup).toHaveBeenCalledWith(9))
  })

  it('opens CSV import on this account from the import hint', async () => {
    listCauses([{ kind: 'staleImport', newestDay: '2026-11-12' }])
    renderModal()

    const user = await typeBankBalance('1012.50')
    expect(await screen.findByText('bankCheck.causes.staleImport (day=12 Nov)')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'bankCheck.causes.importCsv' }))

    expect(mockSetModalState).toHaveBeenCalledWith({ type: 'csvImport', preselectedAccountId: 7 })
  })

  it('says no single row explains the difference when nothing closes it', async () => {
    renderModal()

    await typeBankBalance('1012.50')

    expect(await screen.findByText('bankCheck.causes.none')).toBeInTheDocument()
    expect(screen.queryByRole('listitem')).not.toBeInTheDocument()
  })

  it('never shows the list for an earlier balance under a newer difference', async () => {
    ;(findDifferenceCauses as Mock).mockImplementation(async (_accountId: number, typed: number) => (typed === 101200 ? [{ kind: 'nearCopy', row: copy, copyOf: original }] : []))
    renderModal()

    const user = await typeBankBalance('1012')
    expect(await screen.findByRole('button', { name: 'bankCheck.causes.deleteCopy' })).toBeInTheDocument()
    await user.type(screen.getByLabelText('bankCheck.dialog.bankBalance'), '5')

    expect(screen.queryByText('bankCheck.causes.title')).not.toBeInTheDocument()
    expect(await screen.findByText('bankCheck.causes.none')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'bankCheck.causes.deleteCopy' })).not.toBeInTheDocument()
  })
})
