import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import i18next from 'i18next'
import { I18nextProvider } from 'react-i18next'
import en from '@/i18n/locales/en.yml'
import sk from '@/i18n/locales/sk.yml'
import type { IncomeBudgetPreview } from '@/shared/types'
import { sameCurrency } from '@/shared/testing/snapshotFixtures'
import { monthKey } from '../month'
import { BudgetSummary } from './BudgetSummary'

const NOV = monthKey(2026, 11)
const EUR = { code: 'EUR', minorUnits: 2 }

const preview = (valueMinor: number): IncomeBudgetPreview => ({
  valueMinor,
  valuation: sameCurrency(valueMinor),
  lines: [{ month: NOV, started: true, budgeted: true, receivedMinor: 0, differenceMinor: -612000 }],
})

const renderIn = (language: 'en' | 'sk', valueMinor: number) => {
  const i18n = i18next.createInstance()
  void i18n.init({ resources: { en: { translation: en }, sk: { translation: sk } }, lng: language, fallbackLng: false, interpolation: { escapeValue: false } })
  render(
    <I18nextProvider i18n={i18n}>
      <BudgetSummary preview={preview(valueMinor)} bucketCurrency={EUR} consolidationCurrency={EUR} enabled={true} moment={NOV} day="2026-11-08" />
    </I18nextProvider>,
  )
}

describe('BudgetSummary', () => {
  it('puts the amount into the effect sentence in English', () => {
    renderIn('en', -631500)

    expect(screen.getByText(/^Left to Spend shows/)).toHaveTextContent(
      "Left to Spend shows 6 315.00 EUR more than your cash. November's budget counts from the 1st, and its payment has not arrived yet.",
    )
    expect(screen.getByText('This bucket on 8 November 2026, with the values below')).toBeInTheDocument()
  })

  it('puts the amount into the effect sentence in Slovak', () => {
    renderIn('sk', 25000)

    expect(screen.getByText(/z vašich peňazí/)).toHaveTextContent('250.00 EUR z vašich peňazí zostáva mimo „Zostáva na míňanie“, pretože prišlo viac, než ste rozpočítali.')
  })
})
