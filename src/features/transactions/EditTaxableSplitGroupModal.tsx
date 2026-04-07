import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import type { EventWithData } from '../../shared/types'
import { fromMinorUnits, toMinorUnits } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { updateTaxableSplitGroup } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import TaxableEventSplitEditor from './TaxableEventSplitEditor'
import { SplitLegDraft } from './splitLegDraft'

interface Props {
  splitGroupId: number
  eventType: 'revenue' | 'expense'
  legs: EventWithData[]
  groupNote: string | null
  accountId: number
  onClose: () => void
}

const bpsToPct = (bps: number | null): string => {
  if (bps === null) return ''
  return String(bps / 100)
}

const pctToBps = (str: string): number | null => {
  const trimmed = str.trim()
  if (!trimmed) return null
  const n = parseFloat(trimmed)
  if (isNaN(n)) return null
  return Math.round(n * 100)
}

const legToSplitLegDraft = (leg: EventWithData): SplitLegDraft => ({
  eventId: leg.id,
  eventDate: leg.eventDate,
  amount: fromMinorUnits(leg.amountMinor, leg.currencyMinorUnits),
  vatRate: bpsToPct(leg.vatRateBps),
  vatDeductiblePct: bpsToPct(leg.vatDeductiblePctBps),
  expenseDeductiblePct: bpsToPct(leg.expenseDeductiblePctBps),
  note: leg.note ?? '',
})

const EditTaxableSplitGroupModal = ({ splitGroupId, eventType, legs: originalLegs, groupNote: initialGroupNote, onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()

  const currencyCode = originalLegs[0]?.currencyCode ?? ''
  const currencyMinorUnits = originalLegs[0]?.currencyMinorUnits ?? 2

  const [currentLegs, setCurrentLegs] = useState<SplitLegDraft[]>(() => originalLegs.map(legToSplitLegDraft))
  const [groupNote, setGroupNote] = useState(initialGroupNote ?? '')
  const [submitting, setSubmitting] = useState(false)
  const [minLegsError, setMinLegsError] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    const activeLegs = currentLegs
    if (activeLegs.length < 2) {
      setMinLegsError(true)
      return
    }
    setMinLegsError(false)

    // Build original lookup by eventId
    const originalById = new Map(originalLegs.map((l) => [l.id, l]))

    // Removed: original legs whose eventId no longer appears in currentLegs
    const currentEventIds = new Set(activeLegs.filter((l) => l.eventId !== undefined).map((l) => l.eventId!))
    const removedLegIds = originalLegs.filter((l) => !currentEventIds.has(l.id)).map((l) => l.id)

    // Remaining original legs that are still present
    const updatedLegs = activeLegs
      .filter((l) => l.eventId !== undefined)
      .map((l) => {
        const original = originalById.get(l.eventId!)
        return {
          eventId: l.eventId!,
          amountMinor: toMinorUnits(l.amount, currencyMinorUnits),
          eventDate: l.eventDate,
          note: l.note.trim() || null,
          vatRateBps: pctToBps(l.vatRate),
          vatDeductiblePctBps: eventType === 'expense' ? pctToBps(l.vatDeductiblePct) : null,
          expenseDeductiblePctBps: eventType === 'expense' ? pctToBps(l.expenseDeductiblePct) : null,
          prepaidPeriodMonths: null as number | null,
          _original: original,
        }
      })
      .filter(({ amountMinor, eventDate, note, vatRateBps, vatDeductiblePctBps, expenseDeductiblePctBps, _original }) => {
        if (!_original) return true
        return (
          amountMinor !== _original.amountMinor ||
          eventDate !== _original.eventDate ||
          note !== (_original.note ?? null) ||
          vatRateBps !== _original.vatRateBps ||
          vatDeductiblePctBps !== _original.vatDeductiblePctBps ||
          expenseDeductiblePctBps !== _original.expenseDeductiblePctBps
        )
      })
      .map(({ eventId, amountMinor, eventDate, note, vatRateBps, vatDeductiblePctBps, expenseDeductiblePctBps, prepaidPeriodMonths }) => ({
        eventId,
        amountMinor,
        eventDate,
        note,
        vatRateBps,
        vatDeductiblePctBps,
        expenseDeductiblePctBps,
        prepaidPeriodMonths,
      }))

    const newLegs = activeLegs
      .filter((l) => l.eventId === undefined)
      .map((l) => ({
        amountMinor: toMinorUnits(l.amount, currencyMinorUnits),
        eventDate: l.eventDate,
        note: l.note.trim() || null,
        vatRateBps: pctToBps(l.vatRate),
        vatDeductiblePctBps: eventType === 'expense' ? pctToBps(l.vatDeductiblePct) : null,
        expenseDeductiblePctBps: eventType === 'expense' ? pctToBps(l.expenseDeductiblePct) : null,
        prepaidPeriodMonths: null as number | null,
      }))

    setSubmitting(true)
    try {
      await updateTaxableSplitGroup({
        splitGroupId,
        eventType,
        groupNote: groupNote.trim() || null,
        updatedLegs,
        newLegs,
        removedLegIds,
      })
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
          <DialogTitle>{t('modals.editTaxableSplitGroup.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            {/* Account (read-only) */}
            <div className="flex flex-col gap-2">
              <p className="text-xs text-muted-foreground">{t('modals.editTaxableSplitGroup.accountReadOnly')}</p>
              <p className="text-sm font-medium">{originalLegs[0]?.accountName ?? ''}</p>
            </div>

            <TaxableEventSplitEditor
              eventType={eventType}
              currencyCode={currencyCode}
              currencyMinorUnits={currencyMinorUnits}
              legs={currentLegs}
              onLegsChange={(legs) => {
                setCurrentLegs(legs)
                if (legs.length >= 2) setMinLegsError(false)
              }}
              groupNote={groupNote}
              onGroupNoteChange={setGroupNote}
            />

            {minLegsError && <p className="text-sm text-destructive">{t('modals.editTaxableSplitGroup.minLegsError')}</p>}
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editTaxableSplitGroup.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.editTaxableSplitGroup.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditTaxableSplitGroupModal
