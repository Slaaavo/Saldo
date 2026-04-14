import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { todayIso, toMinorUnits, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { createTaxableEvent, createTaxableSplitGroup, linkCashflowsToTaxable } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import TaxableEventSplitEditor from './TaxableEventSplitEditor'
import ExpenseFormFields from './ExpenseFormFields'
import { SplitLegDraft, makeEmptyLeg } from './splitLegDraft'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useResolvedPersonId } from './useResolvedPersonId'
import PersonPickerField from './PersonPickerField'
import CashflowPicker from './CashflowPicker'

interface Props {
  onClose: () => void
}

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
  const [vatReclaimablePct, setVatReclaimablePct] = useState('')
  const [expenseDeductiblePct, setExpenseDeductiblePct] = useState('')
  const [note, setNote] = useState('')
  const [isSplit, setIsSplit] = useState(false)
  const [legs, setLegs] = useState<SplitLegDraft[]>(() => [makeEmptyLeg(todayIso()), makeEmptyLeg(todayIso())])
  const [groupNote, setGroupNote] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [selectedCashflowIds, setSelectedCashflowIds] = useState<number[]>([])
  const [reclaimedVat, setReclaimedVat] = useState(() => persons.find((p) => p.id === personId)?.vatPayer ?? false)
  const [reclaimedVatPersonId, setReclaimedVatPersonId] = useState(personId)

  if (reclaimedVatPersonId !== personId) {
    setReclaimedVatPersonId(personId)
    const person = persons.find((p) => p.id === personId)
    setReclaimedVat(person?.vatPayer ?? false)
  }

  const amountMinorForFilter = (() => {
    const parsed = parseFloat(amount)
    return !isNaN(parsed) ? -toMinorUnits(amount, currencyMinorUnits) : undefined
  })()

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
        const newEventId = await createTaxableEvent({
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
        if (selectedCashflowIds.length > 0) {
          await linkCashflowsToTaxable(newEventId, selectedCashflowIds).catch((err) => {
            toast.warning(extractErrorMessage(err))
          })
        }
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
                <ExpenseFormFields
                  idPrefix="create-expense"
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
                  reclaimedVat={reclaimedVat}
                  onReclaimedVatChange={setReclaimedVat}
                  currencyCode={currencyCode}
                  currencyMinorUnits={currencyMinorUnits}
                />

                {/* Cashflow picker */}
                {personId !== null && (
                  <CashflowPicker
                    personId={personId}
                    eligibleAmountMinor={amountMinorForFilter}
                    selectedIds={selectedCashflowIds}
                    onToggle={(id) => setSelectedCashflowIds((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]))}
                    currencyMinorUnits={currencyMinorUnits}
                    currencyCode={currencyCode}
                  />
                )}
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
