import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { todayIso, toMinorUnits, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { createTaxableEvent } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useResolvedPersonId } from './useResolvedPersonId'
import PersonPickerField from './PersonPickerField'
import ExpenseFormFields from './ExpenseFormFields'

interface Props {
  onClose: () => void
}

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

            {/* Prepaid until */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-prepaid-expense-until">{t('modals.createPrepaidExpense.prepaidUntil', 'Coverage End Date')}</Label>
              <DatePicker id="create-prepaid-expense-until" value={prepaidUntil || undefined} onChange={(d) => handlePrepaidUntilChange(d)} />
              {prepaidUntilError && <p className="text-xs text-destructive">{prepaidUntilError}</p>}
            </div>

            <ExpenseFormFields
              idPrefix="create-prepaid-expense"
              showReclaimedVat={false}
              amount={amount}
              onAmountChange={setAmount}
              vatRate={vatRate}
              onVatRateChange={setVatRate}
              vatReclaimablePct={vatReclaimablePct}
              onVatReclaimablePctChange={setVatReclaimablePct}
              expenseDeductiblePct={expenseDeductiblePct}
              onExpenseDeductiblePctChange={setExpenseDeductiblePct}
              note={note}
              onNoteChange={setNote}
              reclaimedVat={false}
              onReclaimedVatChange={() => {}}
              currencyCode={currencyCode}
              currencyMinorUnits={currencyMinorUnits}
            />
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
