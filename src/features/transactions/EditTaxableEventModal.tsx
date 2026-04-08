import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import type { EventWithData } from '../../shared/types'
import { fromMinorUnits, toMinorUnits, getMinorUnitsStep, pctToBps, bpsToPct } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { updateTaxableEvent, listLinkedCashflows, listEligibleCashflows, linkCashflowsToTaxable, unlinkCashflowFromTaxable, listPersons } from '../../shared/api'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import NumberValue from '../../shared/ui/NumberValue'

interface Props {
  event: EventWithData
  onClose: () => void
}

const VAT_QUICK_FILLS = ['0', '5', '10', '20', '23']
const VAT_DEDUCTIBLE_QUICK_FILLS = ['100', '50']
const EXPENSE_DEDUCTIBLE_QUICK_FILLS = ['100', '80', '50']

const EditTaxableEventModal = ({ event, onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const isExpense = event.eventType === 'expense'
  const minorUnits = event.currencyMinorUnits

  const [amount, setAmount] = useState(fromMinorUnits(event.amountMinor, minorUnits))
  const [date, setDate] = useState(event.eventDate)
  const [vatRate, setVatRate] = useState(bpsToPct(event.vatRateBps))
  const [vatDeductiblePct, setVatDeductiblePct] = useState(bpsToPct(event.vatDeductiblePctBps))
  const [expenseDeductiblePct, setExpenseDeductiblePct] = useState(bpsToPct(event.expenseDeductiblePctBps))
  const [prepaidPeriodMonths, setPrepaidPeriodMonths] = useState(event.prepaidPeriodMonths !== null ? String(event.prepaidPeriodMonths) : '')
  const [note, setNote] = useState(event.note ?? '')
  const [submitting, setSubmitting] = useState(false)
  const [showAllCashflows, setShowAllCashflows] = useState(false)

  // Resolve person_id from persons list — needed for eligible cashflows query
  const { data: persons = [] } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const personId = persons.find((p) => p.defaultRevenueAccountId === event.accountId || p.defaultExpenseAccountId === event.accountId)?.id ?? null

  const linkedCashflowsQuery = useQuery({
    queryKey: ['linked-cashflows', event.id],
    queryFn: () => listLinkedCashflows(event.id),
  })

  const eligibleCashflowsQuery = useQuery({
    queryKey: ['eligible-cashflows', personId, showAllCashflows, event.amountMinor],
    queryFn: () => (personId !== null ? listEligibleCashflows(personId, showAllCashflows ? undefined : event.amountMinor, true) : Promise.resolve([])),
    enabled: personId !== null,
  })

  const handleLink = async (cashflowId: number) => {
    try {
      await linkCashflowsToTaxable(event.id, [cashflowId])
      await queryClient.invalidateQueries({ queryKey: ['linked-cashflows', event.id] })
      await queryClient.invalidateQueries({ queryKey: ['eligible-cashflows'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
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
    setSubmitting(true)
    try {
      const prepaidInt = prepaidPeriodMonths.trim() ? parseInt(prepaidPeriodMonths) : null
      await updateTaxableEvent({
        eventId: event.id,
        eventType: event.eventType,
        amountMinor: toMinorUnits(amount, minorUnits),
        eventDate: date,
        note: note.trim() || null,
        vatRateBps: pctToBps(vatRate),
        vatDeductiblePctBps: isExpense ? pctToBps(vatDeductiblePct) : null,
        expenseDeductiblePctBps: isExpense ? pctToBps(expenseDeductiblePct) : null,
        prepaidPeriodMonths: isExpense && prepaidInt !== null && !isNaN(prepaidInt) ? prepaidInt : null,
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
  const eligibleCashflows = eligibleCashflowsQuery.data ?? []

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
              <DatePicker value={date} onChange={(d) => setDate(d ?? event.eventDate)} />
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

            {/* Expense-only fields */}
            {isExpense && (
              <>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-taxable-vat-ded">{t('modals.createExpense.vatDeductiblePct')}</Label>
                  <PercentageInput
                    id="edit-taxable-vat-ded"
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

                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-taxable-ded">{t('modals.createExpense.expenseDeductiblePct')}</Label>
                  <PercentageInput
                    id="edit-taxable-ded"
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

                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-taxable-prepaid">{t('modals.createExpense.prepaidPeriodMonths')}</Label>
                  <Input
                    id="edit-taxable-prepaid"
                    type="number"
                    step="1"
                    min={0}
                    value={prepaidPeriodMonths}
                    onChange={(e) => setPrepaidPeriodMonths(e.target.value)}
                    placeholder="0"
                  />
                  <p className="text-xs text-muted-foreground">{t('modals.createExpense.prepaidPeriodHelper')}</p>
                </div>
              </>
            )}

            {/* Note */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-taxable-note">{t('modals.createRevenue.note')}</Label>
              <Input id="edit-taxable-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.createRevenue.notePlaceholder')} />
            </div>

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

              {/* Eligible cashflows picker */}
              {personId !== null && (
                <div className="flex flex-col gap-1">
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-medium text-muted-foreground">{t('taxable.eligibleCashflows')}</span>
                    <button type="button" onClick={() => setShowAllCashflows((v) => !v)} className="text-xs text-primary underline-offset-2 hover:underline">
                      {showAllCashflows ? t('taxable.showMatchingOnly') : t('taxable.showAll')}
                    </button>
                  </div>

                  {eligibleCashflowsQuery.isLoading ? (
                    <p className="text-xs text-muted-foreground">{t('common.loading')}</p>
                  ) : eligibleCashflows.length === 0 ? (
                    <p className="text-xs text-muted-foreground">{t('taxable.noEligibleCashflows')}</p>
                  ) : (
                    <ul className="flex flex-col gap-1 max-h-40 overflow-y-auto">
                      {eligibleCashflows.map((cf) => (
                        <li key={cf.id} className="flex items-center justify-between text-sm gap-2 rounded border px-2 py-1">
                          <span className="text-muted-foreground">{cf.eventDate.slice(0, 10)}</span>
                          <NumberValue value={cf.amountMinor} minorUnits={cf.currencyMinorUnits} currencyCode={cf.currencyCode} className="font-medium tabular-nums" />
                          {cf.note && <span className="flex-1 truncate text-muted-foreground">{cf.note}</span>}
                          <Button type="button" size="sm" variant="outline" onClick={() => handleLink(cf.id)} className="h-6 px-2 text-xs">
                            {t('taxable.linkCashflow')}
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
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
