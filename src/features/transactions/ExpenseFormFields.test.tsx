import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import ExpenseFormFields, { type ExpenseFormFieldsProps } from './ExpenseFormFields'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}))

const SWITCH = 'modals.createExpense.privateUse'

const renderFields = (overrides: Partial<ExpenseFormFieldsProps>) =>
  render(
    <ExpenseFormFields
      idPrefix="test"
      amount="58.30"
      onAmountChange={vi.fn()}
      vatRate="23"
      onVatRateChange={vi.fn()}
      vatReclaimablePct="50"
      onVatReclaimablePctChange={vi.fn()}
      expenseDeductiblePct="80"
      onExpenseDeductiblePctChange={vi.fn()}
      note=""
      onNoteChange={vi.fn()}
      reclaimedVat={true}
      onReclaimedVatChange={vi.fn()}
      privateUse={false}
      onPrivateUseChange={vi.fn()}
      currencyCode="EUR"
      currencyMinorUnits={2}
      {...overrides}
    />,
  )

describe('ExpenseFormFields private-use switch', () => {
  it('shows when part of the VAT is reclaimed', () => {
    renderFields({})

    expect(screen.getByLabelText(SWITCH)).toBeInTheDocument()
  })

  it.each([
    ['all of the VAT is reclaimed', { vatReclaimablePct: '100' }],
    ['none of the VAT is reclaimed', { vatReclaimablePct: '0' }],
    ['the reclaimed share is empty', { vatReclaimablePct: '' }],
    ['the entry has no VAT', { vatRate: '0' }],
    ['the entry does not reclaim VAT', { reclaimedVat: false }],
  ])('hides when %s', (_case, overrides) => {
    renderFields(overrides)

    expect(screen.queryByLabelText(SWITCH)).not.toBeInTheDocument()
  })
})
