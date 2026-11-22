import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../../i18n'
import type { ImportVerdict } from '../../../../shared/types'
import type { ImportRow } from '../types'
import ReviewStep, { type ReviewStepProps } from './ReviewStep'

const makeImportRow = (index: number, date: string, note: string, verdict: ImportVerdict, isSelected: boolean): ImportRow => ({
  index,
  date: `${date}T12:00:00`,
  amountMinor: -1000 * (index + 1),
  note,
  originalCurrencyId: null,
  originalAmountMinor: null,
  fxRateMantissa: null,
  fxRateExponent: null,
  rawIban: null,
  ibanMatch: { type: 'none' },
  verdict,
  isSelected,
  bucketId: null,
  splitLegs: null,
})

const ROWS: ImportRow[] = [
  makeImportRow(0, '2026-03-01', 'Groceries', { kind: 'new' }, true),
  makeImportRow(
    1,
    '2026-03-02',
    'From savings',
    { kind: 'completesTransfer', eventId: 42, accountId: 2, accountName: 'Savings', eventDate: '2026-03-03T12:00:00', note: null },
    true,
  ),
  makeImportRow(2, '2026-03-03', 'Rent', { kind: 'inSaldo', eventId: 40, eventDate: '2026-03-03T12:00:00', note: 'Rent' }, false),
  makeImportRow(3, '2026-03-01', 'To savings', { kind: 'transferLeg', eventId: 41, otherAccountName: 'Savings', eventDate: '2026-02-27T12:00:00' }, false),
]

const reviewProps = (overrides?: Partial<ReviewStepProps>): ReviewStepProps => {
  return {
    importRows: ROWS,
    unparseableRowCount: 0,
    selectedCount: 2,
    canImport: true,
    balanceWarningDates: [],
    availableBuckets: [],
    accountsWithoutIban: [],
    counterpartAccounts: [],
    selectedAccountCurrencyCode: 'EUR',
    selectedAccountMinorUnits: 2,
    importing: false,
    onToggleRow: vi.fn(),
    onSelectAll: vi.fn(),
    onDeselectAll: vi.fn(),
    onBucketChange: vi.fn(),
    onCreatePartner: vi.fn(),
    onAssignIban: vi.fn(),
    onCounterpartChange: vi.fn(),
    onImport: vi.fn(),
    onBack: vi.fn(),
    onCancel: vi.fn(),
    onSplitOpen: vi.fn(),
    onSplitCancel: vi.fn(),
    onLegAmountChange: vi.fn(),
    onLegNoteChange: vi.fn(),
    onLegPartnerChange: vi.fn(),
    onLegBucketChange: vi.fn(),
    onAddLeg: vi.fn(),
    onRemoveLeg: vi.fn(),
    splitValidationErrors: [],
    ...overrides,
  }
}

const renderReview = (overrides?: Partial<ReviewStepProps>) => {
  const props = reviewProps(overrides)
  const view = render(
    <I18nextProvider i18n={i18n}>
      <ReviewStep {...props} />
    </I18nextProvider>,
  )
  const rerender = (changes: Partial<ReviewStepProps>) =>
    view.rerender(
      <I18nextProvider i18n={i18n}>
        <ReviewStep {...props} {...changes} />
      </I18nextProvider>,
    )
  return { props, rerender }
}

describe('ReviewStep', () => {
  it('counts the new rows and the rows already in Saldo', () => {
    renderReview()

    expect(screen.getByText('New rows: 2. Already in Saldo: 2.')).toBeInTheDocument()
  })

  it('folds the rows already in Saldo away under their count until the user opens them', async () => {
    renderReview()
    const disclosure = screen.getByRole('button', { name: '2 already in Saldo' })

    expect(screen.getByText('Groceries')).toBeVisible()
    expect(screen.queryByText('Rent')).not.toBeInTheDocument()
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')

    await userEvent.click(disclosure)

    expect(disclosure).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('Rent')).toBeVisible()
    expect(screen.getByText('To savings')).toBeVisible()
  })

  it('opens the rows already in Saldo from the keyboard', async () => {
    renderReview()

    screen.getByRole('button', { name: '2 already in Saldo' }).focus()
    await userEvent.keyboard('{Enter}')

    expect(screen.getByText('Rent')).toBeVisible()
  })

  it('says which entry a row completes a transfer with', () => {
    renderReview()

    expect(screen.getByText('Becomes one transfer with the entry on Savings dated 2026-03-03')).toBeVisible()
  })

  it("says a transfer leg moves to the row's day, and stays when the row is imported again", async () => {
    const { rerender } = renderReview()
    await userEvent.click(screen.getByRole('button', { name: '2 already in Saldo' }))

    expect(screen.getByText('Transfer with Savings, moves from 2026-02-27 to 2026-03-01')).toBeVisible()

    rerender({ importRows: ROWS.map((row) => (row.index === 3 ? { ...row, isSelected: true } : row)) })

    expect(screen.getByText('Transfer with Savings dated 2026-02-27')).toBeVisible()
  })

  it('counts only the new rows in Select All', async () => {
    const { props } = renderReview()

    await userEvent.click(screen.getByText('Select All (2/2)'))

    expect(props.onDeselectAll).toHaveBeenCalled()
  })

  it('finishes an import with nothing ticked', async () => {
    const { props } = renderReview({ selectedCount: 0 })

    await userEvent.click(screen.getByRole('button', { name: 'Finish' }))

    expect(props.onImport).toHaveBeenCalled()
  })

  it('names the ticked rows on the import button', () => {
    renderReview({ selectedCount: 3 })

    expect(screen.getByRole('button', { name: 'Import 3 rows' })).toBeEnabled()
  })
})
