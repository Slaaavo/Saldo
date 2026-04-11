import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { todayIso, toMinorUnits, getMinorUnitsStep, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { createTaxableEvent } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useResolvedPersonId } from './useResolvedPersonId'
import PersonPickerField from './PersonPickerField'

interface Props {
  onClose: () => void
}

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']
const VAT_RECLAIMABLE_QUICK_FILLS = ['100', '50']
const EXPENSE_DEDUCTIBLE_QUICK_FILLS = ['100', '80', '50']

const CreatePrepaidExpenseModal = ({ onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { personId, resolvedPersonId, localPersonId, setLocalPersonId, persons, showPicker } = useResolvedPersonId()

  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const currencyCode = consolidationCurrencyQuery.data?.code ?? ''
  const currencyMinorUnits = consolidationCurrencyQuery.data?.minorUnits ?? 2

  const [date, setDate] = useState(todayIso())
  const [amount, setAmount] = useState('')
  const [vatRate, setVatRate] = useState('')
  const [vatReclaimablePct, setVatReclaimablePct] = useState('')
  const [expenseDeductiblePct, setExpenseDeductiblePct] = useState('')
  const [prepaidUntil, setPrepaidUntil] = useState('')
  const [note, setNote] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [prepaidUntilError, setPrepaidUntilError] = useState<string | null>(null)

  const validatePrepaidUntil = (expenseDate: string, until: string): boolean => {
    if (!until) return true
    const expenseYear = parseInt(expenseDate.slice(0, 4), 10)
    const untilYear = parseInt(until.slice(0, 4), 10)
    return untilYear > expenseYear
  }

  const handlePrepaidUntilChange = (value: string) => {
    setPrepaidUntil(value)
    if (value && !validatePrepaidUntil(date, value)) {
      setPrepaidUntilError(t('modals.createPrepaidExpense.prepaidUntilYearError', 'Must be in a future calendar year'))
    } else {
      setPrepaidUntilError(null)
    }
  }

  const handleDateChange = (newDate: string) => {
    setDate(newDate)
    if (prepaidUntil && !validatePrepaidUntil(newDate, prepaidUntil)) {
      setPrepaidUntilError(t('modals.createPrepaidExpense.prepaidUntilYearError', 'Must be in a future calendar year'))
    } else {
      setPrepaidUntilError(null)
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!personId) {
      toast.error(t('validation.nameRequired', { entity: t('persons.selector') }))
      return
    }
    const parsed = parseFloat(amount)
    if (isNaN(parsed)) {
      toast.error(t('validation.invalidAmount'))
      return
    }
    if (!prepaidUntil) {
      toast.error(t('modals.createPrepaidExpense.prepaidUntilRequired', 'Coverage end date is required'))
      return
    }
    if (!validatePrepaidUntil(date, prepaidUntil)) {
      setPrepaidUntilError(t('modals.createPrepaidExpense.prepaidUntilYearError', 'Must be in a future calendar year'))
      return
    }
    setSubmitting(true)
    try {
      await createTaxableEvent({
        personId,
        eventType: 'prepaid_expense',
        amountMinor: toMinorUnits(amount, currencyMinorUnits),
        eventDate: date,
        note: note.trim() || null,
        vatRateBps: pctToBps(vatRate),
        vatReclaimablePctBps: pctToBps(vatReclaimablePct),
        expenseDeductiblePctBps: pctToBps(expenseDeductiblePct),
        prepaidUntil: prepaidUntil || null,
        reclaimedVat: null,
      })
      toast.success(t('modals.createPrepaidExpense.success', 'Prepaid expense created'))
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      onClose()
    } catch (err) {
      toast.error(extractErrorMessage(err))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.createPrepaidExpense.title', 'Add Prepaid Expense')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <PersonPickerField showPicker={showPicker} resolvedPersonId={resolvedPersonId} localPersonId={localPersonId} persons={persons} onPersonChange={setLocalPersonId} />

            {/* Date */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createPrepaidExpense.date', 'Date')}</Label>
              <DatePicker value={date} onChange={(d) => handleDateChange(d ?? todayIso())} />
            </div>

            {/* Amount */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-amount">{t('modals.createPrepaidExpense.amount', 'Amount')}</Label>
              <CurrencyInput
                id="create-prepaid-expense-amount"
                type="number"
                step={getMinorUnitsStep(currencyMinorUnits)}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                placeholder="0"
                currencyCode={currencyCode}
                required
              />
            </div>

            {/* Prepaid until */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-until">{t('modals.createPrepaidExpense.prepaidUntil', 'Coverage End Date')}</Label>
              <DatePicker id="create-prepaid-expense-until" value={prepaidUntil || undefined} onChange={(d) => handlePrepaidUntilChange(d)} />
              {prepaidUntilError && <p className="text-xs text-destructive">{prepaidUntilError}</p>}
            </div>

            {/* VAT rate */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-vat">{t('modals.createPrepaidExpense.vatRate', 'VAT Rate')}</Label>
              <PercentageInput
                id="create-prepaid-expense-vat"
                type="number"
                step="0.01"
                min={0}
                max={100}
                value={vatRate}
                onChange={(e) => setVatRate(e.target.value)}
                placeholder={t('modals.createPrepaidExpense.vatRatePlaceholder', 'e.g. 20')}
              />
              <div className="flex gap-1 flex-wrap">
                {VAT_QUICK_FILLS.map((v) => (
                  <button key={v} type="button" onClick={() => setVatRate(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                    {v}%
                  </button>
                ))}
              </div>
            </div>

            {/* VAT reclaimable % */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-vat-ded">{t('modals.createPrepaidExpense.vatReclaimablePct', 'VAT Reclaimable %')}</Label>
              <PercentageInput
                id="create-prepaid-expense-vat-ded"
                type="number"
                step="0.01"
                min={0}
                max={100}
                value={vatReclaimablePct}
                onChange={(e) => setVatReclaimablePct(e.target.value)}
                placeholder={t('modals.createPrepaidExpense.vatReclaimablePctPlaceholder', 'e.g. 100')}
              />
              <div className="flex gap-1 flex-wrap">
                {VAT_RECLAIMABLE_QUICK_FILLS.map((v) => (
                  <button key={v} type="button" onClick={() => setVatReclaimablePct(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                    {v}%
                  </button>
                ))}
              </div>
            </div>

            {/* Expense deductible % */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-ded">{t('modals.createPrepaidExpense.expenseDeductiblePct', 'Expense Deductible %')}</Label>
              <PercentageInput
                id="create-prepaid-expense-ded"
                type="number"
                step="0.01"
                min={0}
                max={100}
                value={expenseDeductiblePct}
                onChange={(e) => setExpenseDeductiblePct(e.target.value)}
                placeholder={t('modals.createPrepaidExpense.expenseDeductiblePctPlaceholder', 'e.g. 100')}
              />
              <div className="flex gap-1 flex-wrap">
                {EXPENSE_DEDUCTIBLE_QUICK_FILLS.map((v) => (
                  <button key={v} type="button" onClick={() => setExpenseDeductiblePct(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                    {v}%
                  </button>
                ))}
              </div>
            </div>

            {/* Note */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-note">{t('modals.createPrepaidExpense.note', 'Note')}</Label>
              <Input
                id="create-prepaid-expense-note"
                type="text"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t('modals.createPrepaidExpense.notePlaceholder', 'e.g. Annual insurance')}
              />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createPrepaidExpense.cancel', 'Cancel')}
            </Button>
            <Button type="submit" disabled={submitting || !!prepaidUntilError}>
              {t('modals.createPrepaidExpense.submit', 'Add Prepaid Expense')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default CreatePrepaidExpenseModal
