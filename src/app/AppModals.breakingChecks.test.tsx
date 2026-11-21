import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import type { ReactNode } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import i18next from 'i18next'
import { I18nextProvider } from 'react-i18next'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import en from '@/i18n/locales/en.yml'
import AppModals from './AppModals'
import { ModalProvider } from './ModalContext'
import { BreakingChecksProvider } from './BreakingChecksContext'
import { SelectedPersonProvider } from './SelectedPersonContext'
import { useModal } from './useModal'
import { makeRow, makeSnapshot } from '@/shared/testing/snapshotFixtures'
import type { BrokenCheck, Currency, EventWithData, ModalState } from '@/shared/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn() } }))

vi.mock('@/shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const tatra: BrokenCheck = { accountId: 3, accountName: 'Tatra', day: '2026-11-14', differenceMinor: -1250, currencyCode: 'EUR', currencyMinorUnits: 2 }
const GUARDED = ['update_event', 'delete_event', 'delete_split_group', 'update_transfer', 'create_balance_update', 'bulk_create_balance_updates']

const balanceUpdate = {
  id: 5,
  accountId: 3,
  accountName: 'Tatra',
  accountType: 'account',
  eventType: 'balance_update',
  eventDate: '2026-11-10T23:59:00',
  amountMinor: 100000,
  note: null,
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
} as EventWithData

const allowedFlags = (command: string) => (invoke as Mock).mock.calls.filter(([name]) => name === command).map(([, args]) => args.allowBreakingChecks)

const Opener = ({ state }: { state: ModalState }) => {
  const { setModalState } = useModal()
  return <button onClick={() => setModalState(state)}>open</button>
}

const Providers = ({ children }: { children: ReactNode }) => {
  const i18n = i18next.createInstance()
  void i18n.init({ resources: { en: { translation: en } }, lng: 'en', fallbackLng: false })
  return (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: 0 } } })}>
        <SelectedPersonProvider>
          <ModalProvider>
            <BreakingChecksProvider>{children}</BreakingChecksProvider>
          </ModalProvider>
        </SelectedPersonProvider>
      </QueryClientProvider>
    </I18nextProvider>
  )
}

const dbLocation = { actionLoading: false, handleConfirmSwitch: vi.fn(), handleLocationChoiceAction: vi.fn(), handleConfirmReset: vi.fn() }

const openModal = async (state: ModalState) => {
  render(
    <Providers>
      <Opener state={state} />
      <AppModals dbLocation={dbLocation} />
    </Providers>,
  )
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'open' }))
  return user
}

const warning = () => screen.findByRole('dialog', { name: 'Change a verified account?' })

beforeEach(() => {
  vi.clearAllMocks()
  ;(invoke as Mock).mockImplementation(async (command: string, args: { allowBreakingChecks?: boolean }) => {
    if (command === 'get_accounts_snapshot') return makeSnapshot({ rows: [makeRow({ accountId: 3, accountName: 'Tatra' })] })
    if (command === 'get_consolidation_currency') return EUR
    if (command === 'get_bulk_update_exclusions') return []
    if (GUARDED.includes(command)) {
      if (!args.allowBreakingChecks) throw { code: 'BREAKS_VERIFIED_CHECKS', message: 'This change makes a verified bank check differ.', checks: [tatra] }
      return null
    }
    throw new Error(`unexpected command ${command}`)
  })
})

describe('a change that would break a verified check', () => {
  it('keeps the balance update open with the typed amount on Cancel, and saves on Change anyway', async () => {
    const user = await openModal({ type: 'editBalanceUpdate', event: balanceUpdate })
    const amount = await screen.findByLabelText('Amount')
    await user.clear(amount)
    await user.type(amount, '990')
    await user.click(screen.getByRole('button', { name: 'Save' }))

    const asked = await warning()
    expect(asked).toHaveTextContent('Tatra matched the bank app on 14 Nov. After this change it differs by 12.50 EUR.')
    expect(asked).toContainElement(document.activeElement as HTMLElement)
    await user.click(within(asked).getByRole('button', { name: 'Cancel' }))

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Change a verified account?' })).not.toBeInTheDocument())
    expect(screen.getByRole('dialog', { name: 'Edit Balance Update' })).toBeInTheDocument()
    expect(screen.getByLabelText('Amount')).toHaveValue(990)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled())
    expect(toast.error).not.toHaveBeenCalled()
    expect(allowedFlags('update_event')).toEqual([false])

    await user.click(screen.getByRole('button', { name: 'Save' }))
    const changeAnyway = within(await warning()).getByRole('button', { name: 'Change anyway' })
    await waitFor(() => expect(changeAnyway).toBeEnabled())
    await user.click(changeAnyway)

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(allowedFlags('update_event')).toEqual([false, false, true])
    expect(invoke).toHaveBeenCalledWith('update_event', {
      input: { eventId: 5, amountMinor: 99000, eventDate: '2026-11-10T23:59:00', note: null },
      allowBreakingChecks: true,
    })
  })

  it('asks above the delete confirm, and Cancel deletes nothing and shows no error', async () => {
    const user = await openModal({ type: 'confirmDeleteEvent', eventId: 42, eventType: 'cashflow' })
    await user.click(await screen.findByRole('button', { name: 'Delete' }))

    const asked = await warning()
    expect(screen.getAllByRole('dialog')).toEqual([asked])
    expect(screen.getByText('Confirm deletion').closest('[role="dialog"]')).toHaveAttribute('aria-hidden', 'true')
    expect(asked).toContainElement(document.activeElement as HTMLElement)
    await user.click(within(asked).getByRole('button', { name: 'Cancel' }))

    await waitFor(() => expect(screen.queryByRole('dialog', { hidden: true })).not.toBeInTheDocument())
    expect(allowedFlags('delete_event')).toEqual([false])
    expect(toast.error).not.toHaveBeenCalled()
  })
})
