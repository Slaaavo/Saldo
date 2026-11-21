import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { Fragment, type ReactElement, type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryCache, QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query'
import FindBankDifferenceModal from './FindBankDifferenceModal'
import { BreakingChecksProvider } from '@/app/BreakingChecksContext'
import { makeRow } from '@/shared/testing/snapshotFixtures'
import { deleteEvent, findDifferenceCauses } from '@/shared/api'
import type { DifferenceCause } from '@/shared/types'

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

vi.mock('@/app/useModal', () => ({
  useModal: () => ({ setModalState: vi.fn(), closeModal: vi.fn(), modalState: { type: 'none' } }),
}))

const toastError = vi.fn()
vi.mock('sonner', () => ({ toast: { success: vi.fn(), info: vi.fn(), error: (...args: unknown[]) => toastError(...args) } }))

vi.mock('@/shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

let rows: { id: number; amount: number }[] = []
const liveBalance = () => 100000 + rows.reduce((sum, row) => sum + row.amount, 0)
let causesMs = 10

const backend = <T,>(answer: () => T, ms = 10): Promise<T> =>
  new Promise((resolve, reject) =>
    setTimeout(() => {
      try {
        resolve(answer())
      } catch (err) {
        reject(err)
      }
    }, ms),
  )

let queryClient: QueryClient

const LedgerQuery = () => {
  useQuery({ queryKey: ['events', 'ledger'], queryFn: () => backend(() => rows.length, 40) })
  return null
}

const DialogOnLiveSnapshot = () => {
  const snapshot = useQuery({
    queryKey: ['snapshot', 'live'],
    queryFn: () => backend(() => makeRow({ accountId: 7, accountName: 'Checking', balanceMinor: liveBalance() })),
  })
  return (
    <>
      <LedgerQuery />
      {snapshot.data ? <FindBankDifferenceModal row={snapshot.data} onClose={vi.fn()} /> : null}
    </>
  )
}

const Providers = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={queryClient}>
    <BreakingChecksProvider>{children}</BreakingChecksProvider>
  </QueryClientProvider>
)

const listedRows = () => screen.queryAllByRole('listitem').map((item) => item.textContent)

beforeEach(() => {
  vi.clearAllMocks()
  rows = []
  causesMs = 10
  ;(deleteEvent as Mock).mockImplementation((id: number) =>
    backend(() => {
      rows = rows.filter((row) => row.id !== id)
    }),
  )
  ;(findDifferenceCauses as Mock).mockImplementation((_accountId: number, bankMinor: number, seenMinor: number) =>
    backend((): DifferenceCause[] => {
      if (seenMinor !== liveBalance()) throw { code: 'CHECK_CHANGED', message: 'The balance changed since you looked. Look at it again.' }
      const gap = bankMinor - liveBalance()
      return rows
        .filter((row) => row.amount === -gap)
        .map((row) => ({
          kind: 'equalsGap',
          row: { target: { kind: 'cashflow', eventId: row.id }, eventDate: '2026-11-12T12:00:00', amountMinor: row.amount, note: `row ${row.id}`, counterpartName: null },
        }))
    }, causesMs),
  )
  queryClient = new QueryClient({
    queryCache: new QueryCache({ onError: (error) => toastError((error as { message: string }).message) }),
    defaultOptions: { queries: { staleTime: Infinity, refetchOnWindowFocus: false, retry: 0 } },
  })
})

const deleteTheListedRowWhile = async (outsideWrite: () => void) => {
  render(<DialogOnLiveSnapshot />, { wrapper: Providers })
  const user = userEvent.setup()
  await user.type(await screen.findByLabelText('bankCheck.dialog.bankBalance'), '1000')
  await user.click(await screen.findByRole('button', { name: 'bankCheck.causes.deleteRow' }))
  outsideWrite()
  await user.click(screen.getByRole('button', { name: 'common.delete' }))
}

describe('FindBankDifferenceModal after a fix while another process wrote', () => {
  it('lists the new row, not the deleted one, when the balance ends where it was', async () => {
    rows = [{ id: 42, amount: -1250 }]

    await deleteTheListedRowWhile(() => {
      rows = [...rows, { id: 44, amount: -1250 }]
    })

    await waitFor(() => expect(listedRows()).toEqual([expect.stringContaining('row 44')]))
    expect(toastError).not.toHaveBeenCalled()
  })

  it('lists the causes of the new gap when they answer after the ledger refresh', async () => {
    rows = [{ id: 42, amount: -1250 }]

    await deleteTheListedRowWhile(() => {
      rows = [...rows, { id: 44, amount: -500 }]
      causesMs = 80
    })

    await waitFor(() => expect(listedRows()).toEqual([expect.stringContaining('row 44')]), { timeout: 3000 })
    expect(toastError).not.toHaveBeenCalled()
  })
})
