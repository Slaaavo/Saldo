import { useTranslation } from 'react-i18next'
import { getMinorUnitsStep } from '../../shared/utils/format'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { Checkbox } from '../../shared/ui/checkbox'

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']
const VAT_RECLAIMABLE_QUICK_FILLS = ['100', '50']
const EXPENSE_DEDUCTIBLE_QUICK_FILLS = ['100', '80', '50']

export interface ExpenseFormFieldsProps {
  idPrefix: string
  amount: string
  onAmountChange: (value: string) => void
  vatRate: string
  onVatRateChange: (value: string) => void
  vatReclaimablePct: string
  onVatReclaimablePctChange: (value: string) => void
  expenseDeductiblePct: string
  onExpenseDeductiblePctChange: (value: string) => void
  note: string
  onNoteChange: (value: string) => void
  reclaimedVat: boolean
  onReclaimedVatChange: (value: boolean) => void
  showReclaimedVat?: boolean
  currencyCode: string
  currencyMinorUnits: number
}

const ExpenseFormFields = ({
  idPrefix,
  amount,
  onAmountChange,
  vatRate,
  onVatRateChange,
  vatReclaimablePct,
  onVatReclaimablePctChange,
  expenseDeductiblePct,
  onExpenseDeductiblePctChange,
  note,
  onNoteChange,
  reclaimedVat,
  onReclaimedVatChange,
  showReclaimedVat = true,
  currencyCode,
  currencyMinorUnits,
}: ExpenseFormFieldsProps) => {
  const { t } = useTranslation()

  return (
    <>
      {/* Amount */}
      <div className="flex flex-col gap-2">
        <Label htmlFor={`${idPrefix}-amount`}>{t('modals.createExpense.amount')}</Label>
        <CurrencyInput
          id={`${idPrefix}-amount`}
          type="number"
          step={getMinorUnitsStep(currencyMinorUnits)}
          value={amount}
          onChange={(e) => onAmountChange(e.target.value)}
          placeholder="0"
          currencyCode={currencyCode}
          required
        />
      </div>

      {/* VAT rate */}
      <div className="flex flex-col gap-2">
        <Label htmlFor={`${idPrefix}-vat`}>{t('modals.createExpense.vatRate')}</Label>
        <PercentageInput
          id={`${idPrefix}-vat`}
          type="number"
          step="0.01"
          min={0}
          max={100}
          value={vatRate}
          onChange={(e) => onVatRateChange(e.target.value)}
          placeholder={t('modals.createExpense.vatRatePlaceholder')}
        />
        <div className="flex gap-1 flex-wrap">
          {VAT_QUICK_FILLS.map((v) => (
            <button key={v} type="button" onClick={() => onVatRateChange(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
              {v}%
            </button>
          ))}
        </div>
      </div>

      {/* VAT reclaimable % */}
      <div className="flex flex-col gap-2">
        <Label htmlFor={`${idPrefix}-vat-ded`}>{t('modals.createExpense.vatReclaimablePct')}</Label>
        <PercentageInput
          id={`${idPrefix}-vat-ded`}
          type="number"
          step="0.01"
          min={0}
          max={100}
          value={vatReclaimablePct}
          onChange={(e) => onVatReclaimablePctChange(e.target.value)}
          placeholder={t('modals.createExpense.vatReclaimablePctPlaceholder')}
        />
        <div className="flex gap-1 flex-wrap">
          {VAT_RECLAIMABLE_QUICK_FILLS.map((v) => (
            <button key={v} type="button" onClick={() => onVatReclaimablePctChange(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
              {v}%
            </button>
          ))}
        </div>
      </div>

      {/* Expense deductible % */}
      <div className="flex flex-col gap-2">
        <Label htmlFor={`${idPrefix}-ded`}>{t('modals.createExpense.expenseDeductiblePct')}</Label>
        <PercentageInput
          id={`${idPrefix}-ded`}
          type="number"
          step="0.01"
          min={0}
          max={100}
          value={expenseDeductiblePct}
          onChange={(e) => onExpenseDeductiblePctChange(e.target.value)}
          placeholder={t('modals.createExpense.expenseDeductiblePctPlaceholder')}
        />
        <div className="flex gap-1 flex-wrap">
          {EXPENSE_DEDUCTIBLE_QUICK_FILLS.map((v) => (
            <button key={v} type="button" onClick={() => onExpenseDeductiblePctChange(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
              {v}%
            </button>
          ))}
        </div>
      </div>

      {/* VAT reclaimed */}
      {showReclaimedVat !== false && (
        <div className="flex items-center gap-2">
          <Checkbox id={`${idPrefix}-reclaimed-vat`} checked={reclaimedVat} onCheckedChange={(v) => onReclaimedVatChange(v === true)} />
          <Label htmlFor={`${idPrefix}-reclaimed-vat`}>{t('modals.createExpense.reclaimedVat')}</Label>
        </div>
      )}

      {/* Note */}
      <div className="flex flex-col gap-2">
        <Label htmlFor={`${idPrefix}-note`}>{t('modals.createExpense.note')}</Label>
        <Input id={`${idPrefix}-note`} type="text" value={note} onChange={(e) => onNoteChange(e.target.value)} placeholder={t('modals.createExpense.notePlaceholder')} />
      </div>
    </>
  )
}

export default ExpenseFormFields
