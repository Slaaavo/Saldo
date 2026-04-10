import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { todayIso, toMinorUnits, getMinorUnitsStep, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { createTaxableEvent, createTaxableSplitGroup, linkCashflowsToTaxable } from '../../shared/api'
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
import CashflowPicker from './CashflowPicker'

interface Props {
  onClose: () => void
}

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']

const CreateRevenueModal = ({ onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { personId, resolvedPersonId, localPersonId, setLocalPersonId, persons, showPicker } = useResolvedPersonId()

  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const currencyCode = consolidationCurrencyQuery.data?.code ?? ''
  const currencyMinorUnits = consolidationCurrencyQuery.data?.minorUnits ?? 2

  const [date, setDate] = useState(todayIso())
  const [amount, setAmount] = useState('')
  const [vatRate, setVatRate] = useState('')
  const [note, setNote] = useState('')
  const [isSplit, setIsSplit] = useState(false)
  const [legs, setLegs] = useState<SplitLegDraft[]>(() => [makeEmptyLeg(todayIso()), makeEmptyLeg(todayIso())])
  const [groupNote, setGroupNote] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [selectedCashflowIds, setSelectedCashflowIds] = useState<number[]>([])

  const amountMinorForFilter = (() => {
    const parsed = parseFloat(amount)
    return !isNaN(parsed) ? toMinorUnits(amount, currencyMinorUnits) : undefined
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
          eventType: 'revenue',
          groupNote: groupNote.trim() || null,
          legs: legs.map((leg) => ({
            amountMinor: toMinorUnits(leg.amount, currencyMinorUnits),
            eventDate: leg.eventDate || date,
            note: leg.note.trim() || null,
            vatRateBps: pctToBps(leg.vatRate),
            vatReclaimablePctBps: null,
            expenseDeductiblePctBps: null,
            prepaidPeriodMonths: null,
            reclaimedVat: null,
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
          eventType: 'revenue',
          amountMinor: toMinorUnits(amount, currencyMinorUnits),
          eventDate: date,
          note: note.trim() || null,
          vatRateBps: pctToBps(vatRate),
          vatReclaimablePctBps: null,
          expenseDeductiblePctBps: null,
          prepaidPeriodMonths: null,
          reclaimedVat: null,
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
          <DialogTitle>{t('modals.createRevenue.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <PersonPickerField showPicker={showPicker} resolvedPersonId={resolvedPersonId} localPersonId={localPersonId} persons={persons} onPersonChange={setLocalPersonId} />

            {/* Date */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createRevenue.date')}</Label>
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
                {t('modals.createRevenue.split')}
              </button>
            </div>

            {isSplit ? (
              <TaxableEventSplitEditor
                eventType="revenue"
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
                  <Label htmlFor="create-revenue-amount">{t('modals.createRevenue.amount')}</Label>
                  <CurrencyInput
                    id="create-revenue-amount"
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
                  <Label htmlFor="create-revenue-vat">{t('modals.createRevenue.vatRate')}</Label>
                  <PercentageInput
                    id="create-revenue-vat"
                    type="number"
                    step="0.01"
                    min={0}
                    max={100}
                    value={vatRate}
                    onChange={(e) => setVatRate(e.target.value)}
                    placeholder={t('modals.createRevenue.vatRatePlaceholder')}
                  />
                  <div className="flex gap-1 flex-wrap">
                    {VAT_QUICK_FILLS.map((v) => (
                      <button key={v} type="button" onClick={() => setVatRate(v)} className="text-xs px-2 py-0.5 rounded bg-muted hover:bg-accent transition-colors">
                        {v}%
                      </button>
                    ))}
                  </div>
                </div>

                {/* Note */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="create-revenue-note">{t('modals.createRevenue.note')}</Label>
                  <Input id="create-revenue-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.createRevenue.notePlaceholder')} />
                </div>

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
              {t('modals.createRevenue.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.createRevenue.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default CreateRevenueModal
