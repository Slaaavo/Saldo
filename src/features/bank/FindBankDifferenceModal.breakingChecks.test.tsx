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
import FindBankDifferenceModal from './FindBankDifferenceModal'
import { BreakingChecksProvider } from '@/app/BreakingChecksContext'
import { ModalProvider } from '@/app/ModalContext'
import { makeRow } from '@/shared/testing/snapshotFixtures'
import type { BrokenCheck, DifferenceCause } from '@/shared/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const savings: BrokenCheck = { accountId: 8, accountName: 'Savings', day: '2026-11-14', differenceMinor: 1250, currencyCode: 'EUR', currencyMinorUnits: 2 }

let liveRowIds: number[] = []

const causesFor = (): DifferenceCause[] =>
  liveRowIds.map((eventId) => ({
    kind: 'equalsGap',
    row: { target: { kind: 'transfer', eventId }, eventDate: '2026-11-12T12:00:00', amountMinor: -1250, note: null, counterpartName: 'Savings' },
  }))

const Providers = ({ children }: { children: ReactNode }) => {
  const i18n = i18next.createInstance()
  void i18n.init({ resources: { en: { translation: en } }, lng: 'en', fallbackLng: false })
  return (
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: 0 } } })}>
        <ModalProvider>
          <BreakingChecksProvider>{children}</BreakingChecksProvider>
        </ModalProvider>
      </QueryClientProvider>
    </I18nextProvider>
  )
}

const deleteCalls = () => (invoke as Mock).mock.calls.filter(([command]) => command === 'delete_event').map(([, args]) => args)

beforeEach(() => {
  vi.clearAllMocks()
  liveRowIds = [42]
  ;(invoke as Mock).mockImplementation(async (command: string, args: { eventId?: number; allowBreakingChecks?: boolean }) => {
    if (command === 'find_difference_causes') return causesFor()
    if (command === 'delete_event') {
      if (!args.allowBreakingChecks) throw { code: 'BREAKS_VERIFIED_CHECKS', message: 'This change makes a verified bank check differ.', checks: [savings] }
      liveRowIds = liveRowIds.filter((id) => id !== args.eventId)
      return null
    }
    throw new Error(`unexpected command ${command}`)
  })
})

describe('Find the difference, when a fix would break a verified check', () => {
  it('asks first, and Cancel closes the confirm and leaves the cause listed', async () => {
    render(<FindBankDifferenceModal row={makeRow({ accountId: 7, accountName: 'Checking', balanceMinor: 100000 })} onClose={vi.fn()} />, { wrapper: Providers })
    const user = userEvent.setup()
    await user.type(screen.getByLabelText('Balance in the bank app'), '1012.50')
    await user.click(await screen.findByRole('button', { name: 'Delete this row' }))
    await user.click(screen.getByRole('button', { name: 'Delete' }))

    const asked = await screen.findByRole('dialog', { name: 'Change a verified account?' })
    expect(asked).toHaveTextContent('Savings matched the bank app on 14 Nov. After this change it differs by 12.50 EUR.')
    await user.click(within(asked).getByRole('button', { name: 'Cancel' }))

    await waitFor(() => expect(screen.queryByText('Confirm deletion')).not.toBeInTheDocument())
    expect(screen.queryByRole('dialog', { name: 'Change a verified account?' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Delete this row' })).toBeInTheDocument()
    expect(screen.getByLabelText('Balance in the bank app')).toHaveValue(1012.5)
    expect(deleteCalls()).toEqual([{ eventId: 42, allowBreakingChecks: false }])
    expect(toast.error).not.toHaveBeenCalled()
    expect(toast.success).not.toHaveBeenCalled()
  })
})
