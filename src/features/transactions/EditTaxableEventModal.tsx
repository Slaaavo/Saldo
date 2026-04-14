import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import type { EventWithData } from '../../shared/types'
import { fromMinorUnits, toMinorUnits, getMinorUnitsStep, pctToBps, bpsToPct } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { updateTaxableEvent, listLinkedCashflows, linkCashflowsToTaxable, unlinkCashflowFromTaxable, listPersons } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import NumberValue from '../../shared/ui/NumberValue'
import CashflowPicker from './CashflowPicker'
import ExpenseFormFields from './ExpenseFormFields'

interface Props {
  event: EventWithData
  onClose: () => void
}

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']

const EditTaxableEventModal = ({ event, onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const isExpense = event.eventType === 'expense'
  const isPrepaidExpense = event.eventType === 'prepaid_expense'
  const minorUnits = event.currencyMinorUnits

  const [amount, setAmount] = useState(fromMinorUnits(event.amountMinor, minorUnits))
  const [date, setDate] = useState(event.eventDate)
  const [vatRate, setVatRate] = useState(bpsToPct(event.vatRateBps))
  const [vatReclaimablePct, setVatReclaimablePct] = useState(bpsToPct(event.vatReclaimablePctBps))
  const [expenseDeductiblePct, setExpenseDeductiblePct] = useState(bpsToPct(event.expenseDeductiblePctBps))
  const [prepaidUntil, setPrepaidUntil] = useState(event.prepaidUntil ?? '')
  const [prepaidUntilError, setPrepaidUntilError] = useState<string | null>(null)
  const [note, setNote] = useState(event.note ?? '')
  const [submitting, setSubmitting] = useState(false)
  const [reclaimedVat, setReclaimedVat] = useState<boolean>(event.reclaimedVat ?? false)

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

  // Resolve person_id from persons list — needed for eligible cashflows query
  const { data: persons = [] } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const personId = persons.find((p) => p.defaultRevenueAccountId === event.accountId || p.defaultExpenseAccountId === event.accountId)?.id ?? null

  const linkedCashflowsQuery = useQuery({
    queryKey: ['linked-cashflows', event.id],
    queryFn: () => listLinkedCashflows(event.id),
  })

  const handleLink = async (cashflowId: number) => {
    try {
      await linkCashflowsToTaxable(event.id, [cashflowId])
      await queryClient.invalidateQueries({ queryKey: ['linked-cashflows', event.id] })
      await queryClient.invalidateQueries({ queryKey: ['eligible-cashflows'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await queryClient.invalidateQueries({ queryKey: ['unmatched-cashflow-count'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  const handleUnlink = async (cashflowId: number) => {
    try {
      await unlinkCashflowFromTaxable(event.id, cashflowId)
      await queryClient.invalidateQueries({ queryKey: ['linked-cashflows', event.id] })
      await queryClient.invalidateQueries({ queryKey: ['eligible-cashflows'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await queryClient.invalidateQueries({ queryKey: ['unmatched-cashflow-count'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    const parsed = parseFloat(amount)
    if (isNaN(parsed)) {
      toast.error(t('validation.invalidAmount'))
      return
    }
    if (isPrepaidExpense) {
      if (!prepaidUntil) {
        toast.error(t('modals.createPrepaidExpense.prepaidUntilRequired', 'Coverage end date is required'))
        return
      }
      if (!validatePrepaidUntil(date, prepaidUntil)) {
        setPrepaidUntilError(t('modals.createPrepaidExpense.prepaidUntilYearError', 'Must be in a future calendar year'))
        return
      }
    }
    setSubmitting(true)
    try {
      await updateTaxableEvent({
        eventId: event.id,
        eventType: event.eventType,
        amountMinor: toMinorUnits(amount, minorUnits),
        eventDate: date,
        note: note.trim() || null,
        vatRateBps: pctToBps(vatRate),
        vatReclaimablePctBps: isExpense || isPrepaidExpense ? pctToBps(vatReclaimablePct) : null,
        expenseDeductiblePctBps: isExpense || isPrepaidExpense ? pctToBps(expenseDeductiblePct) : null,
        prepaidUntil: isPrepaidExpense ? prepaidUntil || null : null,
        reclaimedVat: isExpense ? reclaimedVat : null,
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

  const linkedCashflows = linkedCashflowsQuery.data ?? []

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.editTaxableEvent.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            {/* Account (read-only) */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.editTaxableEvent.accountReadOnly')}</Label>
              <p className="text-sm font-medium">{event.accountName}</p>
            </div>

            {isExpense || isPrepaidExpense ? (
              <>
                {/* Date */}
                <div className="flex flex-col gap-2">
                  <Label>{t('modals.createRevenue.date')}</Label>
                  <DatePicker value={date} onChange={(d) => handleDateChange(d ?? event.eventDate)} />
                </div>

                {/* Prepaid until — only for prepaid expense events */}
                {isPrepaidExpense && (
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="edit-taxable-prepaid-until">{t('modals.createPrepaidExpense.prepaidUntil', 'Coverage End Date')}</Label>
                    <DatePicker id="edit-taxable-prepaid-until" value={prepaidUntil || undefined} onChange={(d) => handlePrepaidUntilChange(d)} />
                    {prepaidUntilError && <p className="text-xs text-destructive">{prepaidUntilError}</p>}
                  </div>
                )}

                <ExpenseFormFields
                  idPrefix="edit-taxable"
                  showReclaimedVat={isExpense}
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
                  currencyCode={event.currencyCode}
                  currencyMinorUnits={minorUnits}
                />
              </>
            ) : (
              <>
                {/* Amount */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-taxable-amount">{t('modals.createRevenue.amount')}</Label>
                  <CurrencyInput
                    id="edit-taxable-amount"
                    type="number"
                    step={getMinorUnitsStep(minorUnits)}
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    placeholder="0"
                    currencyCode={event.currencyCode}
                    required
                  />
                </div>

                {/* Date */}
                <div className="flex flex-col gap-2">
                  <Label>{t('modals.createRevenue.date')}</Label>
                  <DatePicker value={date} onChange={(d) => handleDateChange(d ?? event.eventDate)} />
                </div>

                {/* VAT rate */}
                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-taxable-vat">{t('modals.createRevenue.vatRate')}</Label>
                  <PercentageInput
                    id="edit-taxable-vat"
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
                  <Label htmlFor="edit-taxable-note">{t('modals.createRevenue.note')}</Label>
                  <Input id="edit-taxable-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.createRevenue.notePlaceholder')} />
                </div>
              </>
            )}

            {/* Cashflow links section */}
            <div className="flex flex-col gap-2 pt-2 border-t">
              <Label>{t('taxable.linkedCashflows')}</Label>

              {/* Currently linked cashflows */}
              {linkedCashflows.length > 0 ? (
                <ul className="flex flex-col gap-1">
                  {linkedCashflows.map((cf) => (
                    <li key={cf.id} className="flex items-center justify-between text-sm gap-2 rounded border px-2 py-1">
                      <span className="text-muted-foreground">{cf.eventDate.slice(0, 10)}</span>
                      <NumberValue value={cf.amountMinor} minorUnits={cf.currencyMinorUnits} currencyCode={cf.currencyCode} className="font-medium tabular-nums" />
                      {cf.note && <span className="flex-1 truncate text-muted-foreground">{cf.note}</span>}
                      <Button type="button" size="sm" variant="ghost" onClick={() => handleUnlink(cf.id)} className="h-6 px-2 text-xs text-destructive hover:text-destructive">
                        {t('taxable.unlinkCashflow')}
                      </Button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-xs text-muted-foreground">{t('taxable.noLinkedCashflows')}</p>
              )}

              {/* Cashflow picker */}
              {personId !== null && (
                <CashflowPicker
                  personId={personId}
                  eligibleAmountMinor={isExpense ? -event.amountMinor : event.amountMinor}
                  onLink={handleLink}
                  currencyMinorUnits={minorUnits}
                  currencyCode={event.currencyCode}
                />
              )}
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editTaxableEvent.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.editTaxableEvent.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditTaxableEventModal
