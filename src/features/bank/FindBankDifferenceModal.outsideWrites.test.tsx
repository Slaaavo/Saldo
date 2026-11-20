import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { Fragment, type ReactElement, type ReactNode } from 'react'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryCache, QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query'
import FindBankDifferenceModal from './FindBankDifferenceModal'
import { useRefreshOnExternalWrites } from '@/app/useRefreshOnExternalWrites'
import { makeRow } from '@/shared/testing/snapshotFixtures'
import { findDifferenceCauses, getDbDataVersion } from '@/shared/api'
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
  getDbDataVersion: vi.fn(),
}))

type FocusListener = (event: { payload: boolean }) => void
let focusListener: FocusListener | undefined
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onFocusChanged: async (listener: FocusListener) => {
      focusListener = listener
      return () => {}
    },
  }),
}))

vi.mock('@/app/useModal', () => ({
  useModal: () => ({ setModalState: vi.fn(), closeModal: vi.fn(), modalState: { type: 'none' } }),
}))

const toastError = vi.fn()
vi.mock('sonner', () => ({ toast: { success: vi.fn(), info: vi.fn(), error: (...args: unknown[]) => toastError(...args) } }))

vi.mock('@/shared/ui/NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

let liveBalance = 100000
let dataVersion = 1

const backend = <T,>(answer: () => T): Promise<T> =>
  new Promise((resolve, reject) =>
    setTimeout(() => {
      try {
        resolve(answer())
      } catch (err) {
        reject(err)
      }
    }, 10),
  )

let queryClient: QueryClient

const DialogOnLiveSnapshot = () => {
  useRefreshOnExternalWrites()
  const snapshot = useQuery({
    queryKey: ['snapshot', 'live'],
    queryFn: () => backend(() => makeRow({ accountId: 7, accountName: 'Checking', balanceMinor: liveBalance })),
  })
  return snapshot.data ? <FindBankDifferenceModal row={snapshot.data} onClose={vi.fn()} /> : null
}

const Providers = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

const difference = () => screen.getByText(/bankCheck\.dialog\.(lower|higher|same)/)

const saldoMcpRecordsFiveEuros = () => {
  liveBalance = 100500
  dataVersion = 2
}

beforeEach(() => {
  vi.clearAllMocks()
  focusListener = undefined
  liveBalance = 100000
  dataVersion = 1
  ;(getDbDataVersion as Mock).mockImplementation(() => backend(() => dataVersion))
  ;(findDifferenceCauses as Mock).mockImplementation((_accountId: number, bankMinor: number, seenMinor: number) =>
    backend((): DifferenceCause[] => {
      if (seenMinor !== liveBalance) throw { code: 'CHECK_CHANGED', message: 'The balance changed since you looked. Look at it again.' }
      return bankMinor - liveBalance === 1250
        ? [{ kind: 'equalsGap', row: { target: { kind: 'cashflow', eventId: 42 }, eventDate: '2026-11-12T12:00:00', amountMinor: -1250, note: 'Lidl', counterpartName: null } }]
        : []
    }),
  )
  queryClient = new QueryClient({
    queryCache: new QueryCache({ onError: (error) => toastError((error as { message: string }).message) }),
    defaultOptions: { queries: { staleTime: Infinity, refetchOnWindowFocus: false, retry: 0 } },
  })
})

describe('FindBankDifferenceModal after another process wrote', () => {
  it('moves to the new difference without an error when the window comes back', async () => {
    render(<DialogOnLiveSnapshot />, { wrapper: Providers })
    await waitFor(() => expect(focusListener).toBeDefined())
    await waitFor(() => expect(getDbDataVersion).toHaveBeenCalledTimes(1))
    const user = userEvent.setup()
    await user.type(await screen.findByLabelText('bankCheck.dialog.bankBalance'), '1012.50')
    expect(await screen.findByRole('button', { name: 'bankCheck.causes.deleteRow' })).toBeInTheDocument()

    saldoMcpRecordsFiveEuros()
    await act(async () => focusListener?.({ payload: true }))

    await waitFor(() => expect(difference()).toHaveTextContent('bankCheck.dialog.lower 750'))
    expect(await screen.findByText('bankCheck.causes.none')).toBeInTheDocument()
    expect(toastError).not.toHaveBeenCalled()
  })

  it('catches up with a write made while the window kept focus', async () => {
    render(<DialogOnLiveSnapshot />, { wrapper: Providers })
    await waitFor(() => expect(getDbDataVersion).toHaveBeenCalledTimes(1))
    const input = await screen.findByLabelText('bankCheck.dialog.bankBalance')

    saldoMcpRecordsFiveEuros()
    await userEvent.setup().type(input, '1012.50')

    await waitFor(() => expect(difference()).toHaveTextContent('bankCheck.dialog.lower 750'))
    expect(await screen.findByText('bankCheck.causes.none')).toBeInTheDocument()
    expect(toastError).not.toHaveBeenCalled()
  })
})
