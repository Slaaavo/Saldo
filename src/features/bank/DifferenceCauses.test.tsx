import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import i18next from 'i18next'
import { I18nextProvider } from 'react-i18next'
import en from '@/i18n/locales/en.yml'
import sk from '@/i18n/locales/sk.yml'
import DifferenceCauses from './DifferenceCauses'
import { makeRow } from '@/shared/testing/snapshotFixtures'
import type { DifferenceCause } from '@/shared/types'

vi.mock('@/shared/api', () => ({ deleteEvent: vi.fn(), deleteSplitGroup: vi.fn(), updateEvent: vi.fn() }))
vi.mock('@/app/useModal', () => ({ useModal: () => ({ setModalState: vi.fn(), closeModal: vi.fn(), modalState: { type: 'none' } }) }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const renderCause = (cause: DifferenceCause, lng: 'en' | 'sk') => {
  const i18n = i18next.createInstance()
  void i18n.init({ resources: { en: { translation: en }, sk: { translation: sk } }, lng, fallbackLng: false })
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={new QueryClient()}>
        <DifferenceCauses row={makeRow({ accountId: 7, balanceMinor: 100000 })} causes={[cause]} />
      </QueryClientProvider>
    </I18nextProvider>,
  )
}

const lineWithNote = (note: string) => ({ target: { kind: 'cashflow' as const, eventId: 42 }, eventDate: '2026-11-12T12:00:00', amountMinor: -1250, note, counterpartName: null })

describe('DifferenceCauses confirm', () => {
  for (const note of ['Platba <VS 2026001>', 'Tom & Jerry']) {
    it(`names the row "${note}" as the list does when deleting`, async () => {
      renderCause({ kind: 'equalsGap', row: lineWithNote(note) }, 'en')

      await userEvent.setup().click(screen.getByRole('button', { name: 'Delete this row' }))

      expect(screen.getByRole('dialog')).toHaveTextContent(`Delete the row 12 Nov · ${note} of`)
    })
  }

  it('names the row as the list does when flipping, in Slovak', async () => {
    renderCause({ kind: 'flippedSign', row: lineWithNote('Platba <VS 2026001>') }, 'sk')

    await userEvent.setup().click(screen.getByRole('button', { name: 'Otočiť znamienko' }))

    expect(screen.getByRole('dialog')).toHaveTextContent('Zmeniť záznam 12. 11. · Platba <VS 2026001> z')
  })
})
