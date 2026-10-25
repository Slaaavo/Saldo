import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { AsOf } from '../../shared/types'
import BulkUpdateBalanceModal from './BulkUpdateBalanceModal'
import { SelectedPersonProvider } from '../../app/SelectedPersonContext'
import { makeRow, makeSnapshot } from '../../shared/testing/snapshotFixtures'

vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

vi.mock('sonner', () => ({
  toast: { error: vi.fn() },
}))

vi.mock('../../shared/api', () => ({
  getAccountsSnapshot: vi.fn(),
}))
import { getAccountsSnapshot } from '../../shared/api'

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

const TODAY = '2026-01-15T23:59:00'
const EARLIER = '2026-01-10T23:59:00'

const balancesByMoment: Record<string, number> = { [TODAY]: 123456, [EARLIER]: 50000 }

const renderModal = (props?: { exclusions?: number[]; onSubmit?: Mock }) => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  return render(
    <QueryClientProvider client={queryClient}>
      <SelectedPersonProvider>
        <BulkUpdateBalanceModal exclusions={props?.exclusions ?? []} onSubmit={props?.onSubmit ?? vi.fn()} onClose={vi.fn()} />
      </SelectedPersonProvider>
    </QueryClientProvider>,
  )
}

describe('BulkUpdateBalanceModal', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date(2026, 0, 15, 10, 0, 0))
    ;(getAccountsSnapshot as Mock).mockImplementation(async (asOf: AsOf) => {
      const until = asOf.kind === 'at' ? asOf.until : ''
      return makeSnapshot({
        rows: [
          makeRow({ accountId: 1, accountName: 'Checking', balanceMinor: balancesByMoment[until] ?? 0 }),
          makeRow({ accountId: 2, accountName: 'Savings', balanceMinor: 900 }),
        ],
      })
    })
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it('saves an untouched date with the time the date picker shows', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn().mockResolvedValue(undefined)
    renderModal({ onSubmit })

    expect(screen.getByTestId('date-picker')).toHaveValue(TODAY)
    const [checkingInput] = await screen.findAllByRole('spinbutton')
    await user.type(checkingInput, '42')
    await user.click(screen.getByRole('button', { name: 'modals.bulkUpdate.submit' }))

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith([{ accountId: 1, amountMinor: 4200 }], TODAY, ''))
  })

  it('shows each account balance as of the date in the dialog', async () => {
    renderModal()

    expect(await screen.findByText('1 234.56 EUR')).toBeInTheDocument()
    expect(getAccountsSnapshot).toHaveBeenCalledWith({ kind: 'at', until: TODAY }, undefined)
  })

  it('shows the balance as of the new date when the date changes', async () => {
    renderModal()
    await screen.findByText('1 234.56 EUR')

    fireEvent.change(screen.getByTestId('date-picker'), { target: { value: EARLIER } })

    expect(await screen.findByText('500.00 EUR')).toBeInTheDocument()
    expect(getAccountsSnapshot).toHaveBeenCalledWith({ kind: 'at', until: EARLIER }, undefined)
  })

  it('leaves out accounts hidden from bulk updates', async () => {
    renderModal({ exclusions: [1] })

    expect(await screen.findByText('Savings')).toBeInTheDocument()
    expect(screen.queryByText('Checking')).not.toBeInTheDocument()
  })
})
