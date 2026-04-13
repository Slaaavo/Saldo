import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { todayIso, toMinorUnits, getMinorUnitsStep, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { createTaxableEvent, createTaxableSplitGroup } from '../../shared/api'
import { DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { Checkbox } from '../../shared/ui/checkbox'
import { DatePicker } from '../../shared/ui/date-picker'
import TaxableEventSplitEditor from '../transactions/TaxableEventSplitEditor'
import type { SplitLegDraft } from '../transactions/splitLegDraft'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useResolvedPersonId } from '../transactions/useResolvedPersonId'

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']
const VAT_RECLAIMABLE_QUICK_FILLS = ['100', '50']
const EXPENSE_DEDUCTIBLE_QUICK_FILLS = ['100', '80', '50']

interface ReviewStepProps {
  processedLegs: SplitLegDraft[]
  vendorName: string
  receiptDate: string
  onImportSuccess: () => void
  onBack: () => void
}

const ReviewStep = ({ processedLegs, vendorName, receiptDate, onImportSuccess, onBack }: ReviewStepProps) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { personId } = useResolvedPersonId()
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const currencyCode = consolidationCurrencyQuery.data?.code ?? ''
  const currencyMinorUnits = consolidationCurrencyQuery.data?.minorUnits ?? 2

  const firstLeg = processedLegs[0]
  const initialDate = receiptDate || firstLeg?.eventDate || todayIso()

  // Single-leg form state
  const [date, setDate] = useState(initialDate)
  const [amount, setAmount] = useState(firstLeg?.amount ?? '')
  const [vatRate, setVatRate] = useState(firstLeg?.vatRate ?? '')
  const [vatReclaimablePct, setVatReclaimablePct] = useState(firstLeg?.vatReclaimablePct ?? '')
  const [expenseDeductiblePct, setExpenseDeductiblePct] = useState(firstLeg?.expenseDeductiblePct ?? '')
  const [note, setNote] = useState(firstLeg?.note ?? '')
  const [reclaimedVat, setReclaimedVat] = useState(firstLeg?.reclaimedVat === true)

  // Multi-leg form state
  const [multiDate, setMultiDate] = useState(initialDate)
  const [groupNote, setGroupNote] = useState(vendorName)
  const [legs, setLegs] = useState<SplitLegDraft[]>(processedLegs)

  const [submitting, setSubmitting] = useState(false)

  const handleLegsChange = (newLegs: SplitLegDraft[]) => {
    setLegs(newLegs.map((l) => ({ ...l, eventDate: l.eventDate || multiDate })))
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!personId) {
      toast.error(t('validation.nameRequired', { entity: t('persons.selector') }))
      return
    }
    setSubmitting(true)
    try {
      if (processedLegs.length >= 2) {
        await createTaxableSplitGroup({
          personId,
          eventType: 'expense',
          groupNote: groupNote.trim() || null,
          legs: legs.map((leg) => ({
            amountMinor: toMinorUnits(leg.amount, currencyMinorUnits),
            eventDate: leg.eventDate || multiDate,
            note: leg.note.trim() || null,
            vatRateBps: pctToBps(leg.vatRate),
            vatReclaimablePctBps: pctToBps(leg.vatReclaimablePct),
            expenseDeductiblePctBps: pctToBps(leg.expenseDeductiblePct),
            prepaidUntil: null,
            reclaimedVat: leg.reclaimedVat,
          })),
        })
      } else {
        const parsed = parseFloat(amount)
        if (isNaN(parsed)) {
          toast.error(t('validation.invalidAmount'))
          setSubmitting(false)
          return
        }
        await createTaxableEvent({
          personId,
          eventType: 'expense',
          amountMinor: toMinorUnits(amount, currencyMinorUnits),
          eventDate: date,
          note: note.trim() || null,
          vatRateBps: pctToBps(vatRate),
          vatReclaimablePctBps: pctToBps(vatReclaimablePct),
          expenseDeductiblePctBps: pctToBps(expenseDeductiblePct),
          prepaidUntil: null,
          reclaimedVat,
        })
      }
      setSubmitting(false)
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await onImportSuccess()
    } catch (err) {
      toast.error(extractErrorMessage(err))
      setSubmitting(false)
    }
  }

  if (processedLegs.length === 0) {
    return (
      <>
        <DialogBody>
          <p className="text-sm text-muted-foreground">{t('ekasaImport.reviewStep.noItems')}</p>
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onBack}>
            {t('ekasaImport.reviewStep.back')}
          </Button>
        </DialogFooter>
      </>
    )
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
      <DialogBody className="flex flex-col gap-4">
        {processedLegs.length === 1 ? (
          <>
            {/* Date */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createExpense.date')}</Label>
              <DatePicker value={date} onChange={(d) => setDate(d ?? todayIso())} />
            </div>

            {/* Amount */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="review-expense-amount">{t('modals.createExpense.amount')}</Label>
              <CurrencyInput
                id="review-expense-amount"
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
              <Label htmlFor="review-expense-vat">{t('modals.createExpense.vatRate')}</Label>
              <PercentageInput
                id="review-expense-vat"
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

            {/* VAT reclaimable % */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="review-expense-vat-ded">{t('modals.createExpense.vatReclaimablePct')}</Label>
              <PercentageInput
                id="review-expense-vat-ded"
                type="number"
                step="0.01"
                min={0}
                max={100}
                value={vatReclaimablePct}
                onChange={(e) => setVatReclaimablePct(e.target.value)}
                placeholder={t('modals.createExpense.vatReclaimablePctPlaceholder')}
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
              <Label htmlFor="review-expense-ded">{t('modals.createExpense.expenseDeductiblePct')}</Label>
              <PercentageInput
                id="review-expense-ded"
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

            {/* Note */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="review-expense-note">{t('modals.createExpense.note')}</Label>
              <Input id="review-expense-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.createExpense.notePlaceholder')} />
            </div>

            {/* VAT reclaimed */}
            <div className="flex items-center gap-2">
              <Checkbox id="review-expense-reclaimed-vat" checked={reclaimedVat} onCheckedChange={(v) => setReclaimedVat(v === true)} />
              <Label htmlFor="review-expense-reclaimed-vat">{t('modals.createExpense.reclaimedVat')}</Label>
            </div>
          </>
        ) : (
          <>
            {/* Global date (used as fallback for legs without a date) */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createExpense.date')}</Label>
              <DatePicker value={multiDate} onChange={(d) => setMultiDate(d ?? todayIso())} />
            </div>

            {/* Split editor — renders group note field and per-leg reclaimed VAT internally */}
            <TaxableEventSplitEditor
              eventType="expense"
              currencyCode={currencyCode}
              currencyMinorUnits={currencyMinorUnits}
              legs={legs}
              onLegsChange={handleLegsChange}
              groupNote={groupNote}
              onGroupNoteChange={setGroupNote}
            />
          </>
        )}
      </DialogBody>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack}>
          {t('ekasaImport.reviewStep.back')}
        </Button>
        <Button type="submit" disabled={submitting}>
          {t('ekasaImport.reviewStep.save')}
        </Button>
      </DialogFooter>
    </form>
  )
}

export default ReviewStep
