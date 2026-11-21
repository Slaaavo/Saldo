import { describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import i18next from 'i18next'
import { I18nextProvider } from 'react-i18next'
import en from '@/i18n/locales/en.yml'
import sk from '@/i18n/locales/sk.yml'
import BreakingChecksDialog from './BreakingChecksDialog'
import type { BrokenCheck } from '@/shared/types'

const tatra: BrokenCheck = { accountId: 3, accountName: 'Tatra', day: '2026-11-14', differenceMinor: -1250, currencyCode: 'EUR', currencyMinorUnits: 2 }
const savings: BrokenCheck = { accountId: 4, accountName: 'Savings <joint>', day: '2026-11-02', differenceMinor: 500, currencyCode: 'EUR', currencyMinorUnits: 2 }

const renderDialog = (lng: 'en' | 'sk', onConfirm = vi.fn(), onCancel = vi.fn()) => {
  const i18n = i18next.createInstance()
  void i18n.init({ resources: { en: { translation: en }, sk: { translation: sk } }, lng, fallbackLng: false })
  render(
    <I18nextProvider i18n={i18n}>
      <BreakingChecksDialog checks={[tatra, savings]} onConfirm={onConfirm} onCancel={onCancel} />
    </I18nextProvider>,
  )
  return { onConfirm, onCancel }
}

describe('BreakingChecksDialog', () => {
  it('names every broken check with its day and the new difference', () => {
    renderDialog('en')

    const dialog = screen.getByRole('dialog', { name: 'Change a verified account?' })
    expect(dialog).toHaveTextContent('Tatra matched the bank app on 14 Nov. After this change it differs by 12.50 EUR.')
    expect(dialog).toHaveTextContent('Savings <joint> matched the bank app on 2 Nov. After this change it differs by 5.00 EUR.')
  })

  it('names every broken check in Slovak', () => {
    renderDialog('sk')

    const dialog = screen.getByRole('dialog', { name: 'Zmeniť overený účet?' })
    expect(dialog).toHaveTextContent('Účet Tatra sa 14. 11. zhodoval s aplikáciou banky. Po tejto zmene sa bude líšiť o 12.50 EUR.')
    expect(dialog).toHaveTextContent('Účet Savings <joint> sa 2. 11. zhodoval s aplikáciou banky. Po tejto zmene sa bude líšiť o 5.00 EUR.')
    expect(screen.getByRole('button', { name: 'Aj tak zmeniť' })).toBeInTheDocument()
  })

  it('makes the change only on Change anyway', async () => {
    const { onConfirm, onCancel } = renderDialog('en')
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(onCancel).toHaveBeenCalledOnce()
    expect(onConfirm).not.toHaveBeenCalled()

    const changeAnyway = screen.getByRole('button', { name: 'Change anyway' })
    await waitFor(() => expect(changeAnyway).toBeEnabled())
    await user.click(changeAnyway)
    expect(onConfirm).toHaveBeenCalledOnce()
  })

  it('ignores Change anyway until a double-click on the button beneath it is over', () => {
    vi.useFakeTimers()
    try {
      const { onConfirm } = renderDialog('en')
      const changeAnyway = screen.getByRole('button', { name: 'Change anyway' })

      fireEvent.click(changeAnyway)
      act(() => vi.advanceTimersByTime(500))
      fireEvent.click(changeAnyway)
      expect(onConfirm).not.toHaveBeenCalled()

      act(() => vi.advanceTimersByTime(100))
      fireEvent.click(changeAnyway)
      expect(onConfirm).toHaveBeenCalledOnce()
    } finally {
      vi.useRealTimers()
    }
  })
})
