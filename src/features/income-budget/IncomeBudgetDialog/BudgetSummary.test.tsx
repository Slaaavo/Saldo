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

    expect(screen.getByText(/^This bucket raises/)).toHaveTextContent(
      "This bucket raises Left to Spend by 6 315.00 EUR. November's budget counts from the 1st, and its payment has not arrived yet.",
    )
    expect(screen.getByText('This bucket on 8 November 2026, with the values below')).toBeInTheDocument()
  })

  it('puts the amount into the effect sentence in Slovak', () => {
    renderIn('sk', 25000)

    expect(screen.getByText(/^Táto obálka znižuje/)).toHaveTextContent('Táto obálka znižuje „Zostáva na míňanie“ o 250.00 EUR, pretože prišlo viac, než ste rozpočítali.')
  })

  it('says the bucket leaves Left to Spend unchanged at zero', () => {
    renderIn('en', 0)

    expect(screen.getByText('This bucket leaves Left to Spend unchanged.')).toBeInTheDocument()
  })
})
