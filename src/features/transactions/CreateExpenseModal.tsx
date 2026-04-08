import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { todayIso, toMinorUnits, getMinorUnitsStep, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { createTaxableEvent, createTaxableSplitGroup } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import TaxableEventSplitEditor from './TaxableEventSplitEditor'
import { SplitLegDraft, makeEmptyLeg } from './splitLegDraft'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useResolvedPersonId } from './useResolvedPersonId'
import PersonPickerField from './PersonPickerField'

interface Props {
  onClose: () => void
}

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']
const VAT_DEDUCTIBLE_QUICK_FILLS = ['100', '50']
const EXPENSE_DEDUCTIBLE_QUICK_FILLS = ['100', '80', '50']

const CreateExpenseModal = ({ onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { personId, resolvedPersonId, localPersonId, setLocalPersonId, persons, showPicker } = useResolvedPersonId()

  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const currencyCode = consolidationCurrencyQuery.data?.code ?? ''
  const currencyMinorUnits = consolidationCurrencyQuery.data?.minorUnits ?? 2

  const [date, setDate] = useState(todayIso())
  const [amount, setAmount] = useState('')
  const [vatRate, setVatRate] = useState('')
  const [vatDeductiblePct, setVatDeductiblePct] = useState('')
  const [expenseDeductiblePct, setExpenseDeductiblePct] = useState('')
  const [prepaidPeriodMonths, setPrepaidPeriodMonths] = useState('')
  const [note, setNote] = useState('')
  const [isSplit, setIsSplit] = useState(false)
  const [legs, setLegs] = useState<SplitLegDraft[]>(() => [makeEmptyLeg(todayIso()), makeEmptyLeg(todayIso())])
  const [groupNote, setGroupNote] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const handleLegsChange = (newLegs: SplitLegDraft[]) => {
    setLegs(newLegs.map((l) => ({ ...l, eventDate: l.eventDate || date })))
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!personId) {
      toast.error(t('validation.nameRequired', { entity: t('persons.selector') }))
      return
    }
    setSubmitting(true)
    try {
      if (isSplit) {
        if (legs.length < 2) {
          toast.error(t('modals.editTaxableSplitGroup.minLegsError'))
          setSubmitting(false)
          return
        }
        await createTaxableSplitGroup({
          personId,
          eventType: 'expense',
          groupNote: groupNote.trim() || null,
          legs: legs.map((leg) => ({
            amountMinor: toMinorUnits(leg.amount, currencyMinorUnits),
            eventDate: leg.eventDate || date,
            note: leg.note.trim() || null,
            vatRateBps: pctToBps(leg.vatRate),
            vatDeductiblePctBps: pctToBps(leg.vatDeductiblePct),
            expenseDeductiblePctBps: pctToBps(leg.expenseDeductiblePct),
            prepaidPeriodMonths: null,
          })),
        })
      } else {
        const parsed = parseFloat(amount)
        if (isNaN(parsed)) {
          toast.error(t('validation.invalidAmount'))
          setSubmitting(false)
          return
        }
        const prepaidInt = prepaidPeriodMonths.trim() ? parseInt(prepaidPeriodMonths) : null
        await createTaxableEvent({
          personId,
          eventType: 'expense',
          amountMinor: toMinorUnits(amount, currencyMinorUnits),
          eventDate: date,
          note: note.trim() || null,
          vatRateBps: pctToBps(vatRate),
          vatDeductiblePctBps: pctToBps(vatDeductiblePct),
          expenseDeductiblePctBps: pctToBps(expenseDeductiblePct),
          prepaidPeriodMonths: prepaidInt !== null && !isNaN(prepaidInt) ? prepaidInt : null,
        })
      }
      setSubmitting(false)
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      onClose()
    } catch (err) {
      toast.error(extractErrorMessage(err))
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
          <DialogTitle>{t('modals.createExpense.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <PersonPickerField showPicker={showPicker} resolvedPersonId={resolvedPersonId} localPersonId={localPersonId} persons={persons} onPersonChange={setLocalPersonId} />

            {/* Date */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createExpense.date')}</Label>
              <DatePicker value={date} onChange={(d) => setDate(d ?? todayIso())} />
            </div>

            {/* Split toggle */}
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setIsSplit((v) => !v)}
                className={`px-3 py-1 rounded-md text-sm font-medium border transition-colors ${
                  isSplit ? 'bg-primary text-primary-foreground border-primary' : 'bg-background text-muted-foreground border-border hover:bg-muted'
                }`}
              >
                {t('modals.createExpense.split')}
              </button>
            </div>

            {isSplit ? (
              <TaxableEventSplitEditor
                eventType="expense"
                currencyCode={currencyCode}
                currencyMinorUnits={currencyMinorUnits}
                legs={legs}
                onLegsChange={handleLegsChange}
                groupNote={groupNote}
                onGroupNoteChange={setGroupNote}
              />
            ) : (
              <>
                {/* Amount */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-expense-amount">{t('modals.createExpense.amount')}</Label>
                  <CurrencyInput
                    id="create-expense-amount"
                    type="number"
                    step={getMinorUnitsStep(currencyMinorUnits)}
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    placeholder="0"
                    currencyCode={currencyCode}
                    required
                  />
                </div>

                {/* VAT rate */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-expense-vat">{t('modals.createExpense.vatRate')}</Label>
                  <PercentageInput
                    id="create-expense-vat"
                    type="number"
                    step="0.01"
                    min={0}
                    max={100}
                    value={vatRate}
                    onChange={(e) => setVatRate(e.target.value)}
                    placeholder={t('modals.createExpense.vatRatePlaceholder')}
                  />
                  <div className="flex gap-1 flex-wrap">
                    {VAT_QUICK_FILLS.map((v) => (
                      <button key={v} type="button" onClick={() => setVatRate(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                        {v}%
                      </button>
                    ))}
                  </div>
                </div>

                {/* VAT deductible % */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-expense-vat-ded">{t('modals.createExpense.vatDeductiblePct')}</Label>
                  <PercentageInput
                    id="create-expense-vat-ded"
                    type="number"
                    step="0.01"
                    min={0}
                    max={100}
                    value={vatDeductiblePct}
                    onChange={(e) => setVatDeductiblePct(e.target.value)}
                    placeholder={t('modals.createExpense.vatDeductiblePctPlaceholder')}
                  />
                  <div className="flex gap-1 flex-wrap">
                    {VAT_DEDUCTIBLE_QUICK_FILLS.map((v) => (
                      <button key={v} type="button" onClick={() => setVatDeductiblePct(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                        {v}%
                      </button>
                    ))}
                  </div>
                </div>

                {/* Expense deductible % */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-expense-ded">{t('modals.createExpense.expenseDeductiblePct')}</Label>
                  <PercentageInput
                    id="create-expense-ded"
                    type="number"
                    step="0.01"
                    min={0}
                    max={100}
                    value={expenseDeductiblePct}
                    onChange={(e) => setExpenseDeductiblePct(e.target.value)}
                    placeholder={t('modals.createExpense.expenseDeductiblePctPlaceholder')}
                  />
                  <div className="flex gap-1 flex-wrap">
                    {EXPENSE_DEDUCTIBLE_QUICK_FILLS.map((v) => (
                      <button key={v} type="button" onClick={() => setExpenseDeductiblePct(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                        {v}%
                      </button>
                    ))}
                  </div>
                </div>

                {/* Prepaid period */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-expense-prepaid">{t('modals.createExpense.prepaidPeriodMonths')}</Label>
                  <Input
                    id="create-expense-prepaid"
                    type="number"
                    step="1"
                    min={0}
                    value={prepaidPeriodMonths}
                    onChange={(e) => setPrepaidPeriodMonths(e.target.value)}
                    placeholder="0"
                  />
                  <p className="text-xs text-muted-foreground">{t('modals.createExpense.prepaidPeriodHelper')}</p>
                </div>

                {/* Note */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-expense-note">{t('modals.createExpense.note')}</Label>
                  <Input id="create-expense-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.createExpense.notePlaceholder')} />
                </div>
              </>
            )}
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createExpense.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.createExpense.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default CreateExpenseModal
