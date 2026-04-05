import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import Decimal from 'decimal.js'
import type { EventWithData } from '../../shared/types'
import { fromMinorUnits, toMinorUnits, getMinorUnitsStep } from '../../shared/utils/format'
import { parseRateInput } from '../currency/fxRate'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'

type UpdateTransferPayload = {
  fromEventId: number
  toEventId: number
  fromDate: string
  toDate: string
  amountMinor: number
  toAmountMinor: number
  note: string | null
  originalCurrencyId: number | null
  fxRateMantissa: number | null
  fxRateExponent: number | null
}

interface EditTransferModalProps {
  fromEvent: EventWithData
  toEvent: EventWithData
  onSubmit: (payload: UpdateTransferPayload) => Promise<void>
  onClose: () => void
}

const EditTransferModal = ({ fromEvent, toEvent, onSubmit, onClose }: EditTransferModalProps) => {
  const { t } = useTranslation()
  const [fromDate, setFromDate] = useState(fromEvent.eventDate)
  const [toDate, setToDate] = useState(toEvent.eventDate)
  const [fromAmountStr, setFromAmountStr] = useState(fromMinorUnits(Math.abs(fromEvent.amountMinor), fromEvent.currencyMinorUnits))
  const [toAmountStr, setToAmountStr] = useState(fromMinorUnits(toEvent.amountMinor, toEvent.currencyMinorUnits))
  const [note, setNote] = useState(fromEvent.note ?? '')
  const [submitting, setSubmitting] = useState(false)

  const isCrossCurrency = fromEvent.originalCurrencyId !== null
  const fromCurrencyCode = fromEvent.currencyCode
  const toCurrencyCode = toEvent.currencyCode

  let fxRateDisplay: string | null = null
  if (isCrossCurrency) {
    try {
      const fromVal = new Decimal(fromAmountStr)
      if (!fromVal.isZero()) {
        fxRateDisplay = new Decimal(toAmountStr).div(fromVal).toSignificantDigits(6).toString()
      }
    } catch {
      fxRateDisplay = null
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    const parsedFrom = parseFloat(fromAmountStr)
    if (isNaN(parsedFrom) || parsedFrom <= 0) {
      toast.error(t('validation.invalidAmount'))
      return
    }

    if (isCrossCurrency) {
      const parsedTo = parseFloat(toAmountStr)
      if (isNaN(parsedTo) || parsedTo <= 0) {
        toast.error(t('validation.invalidAmount'))
        return
      }
      const parsedRate = parseRateInput(fxRateDisplay ?? '')
      if (!parsedRate) {
        toast.error(t('fxRates.invalidRate'))
        return
      }
      const payload: UpdateTransferPayload = {
        fromEventId: fromEvent.id,
        toEventId: toEvent.id,
        fromDate,
        toDate,
        amountMinor: -toMinorUnits(fromAmountStr, fromEvent.currencyMinorUnits),
        toAmountMinor: toMinorUnits(toAmountStr, toEvent.currencyMinorUnits),
        note: note.trim() || null,
        originalCurrencyId: fromEvent.originalCurrencyId,
        fxRateMantissa: parsedRate.mantissa,
        fxRateExponent: parsedRate.exponent,
      }
      setSubmitting(true)
      try {
        await onSubmit(payload)
      } finally {
        setSubmitting(false)
      }
    } else {
      const payload: UpdateTransferPayload = {
        fromEventId: fromEvent.id,
        toEventId: toEvent.id,
        fromDate,
        toDate,
        amountMinor: -toMinorUnits(fromAmountStr, fromEvent.currencyMinorUnits),
        toAmountMinor: toMinorUnits(fromAmountStr, fromEvent.currencyMinorUnits),
        note: note.trim() || null,
        originalCurrencyId: null,
        fxRateMantissa: null,
        fxRateExponent: null,
      }
      setSubmitting(true)
      try {
        await onSubmit(payload)
      } finally {
        setSubmitting(false)
      }
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
          <DialogTitle>{t('modals.editTransfer.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            {/* From / To columns */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="flex flex-col gap-2">
                <Label>{t('modals.editTransfer.from')}</Label>
                <Input type="text" value={fromEvent.accountName} disabled className="bg-muted" />
                <Label htmlFor="et-from-date">{t('modals.editTransfer.fromDate')}</Label>
                <DatePicker id="et-from-date" value={fromDate} onChange={setFromDate} withTime defaultTime="23:59" />
              </div>
              <div className="flex flex-col gap-2">
                <Label>{t('modals.editTransfer.to')}</Label>
                <Input type="text" value={toEvent.accountName} disabled className="bg-muted" />
                <Label htmlFor="et-to-date">{t('modals.editTransfer.toDate')}</Label>
                <DatePicker id="et-to-date" value={toDate} onChange={setToDate} withTime defaultTime="23:59" />
              </div>
            </div>

            {/* Amount section */}
            {isCrossCurrency ? (
              <>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="et-from-amount">{t('modals.editTransfer.fromAmount')}</Label>
                    <CurrencyInput
                      id="et-from-amount"
                      type="number"
                      step={getMinorUnitsStep(fromEvent.currencyMinorUnits)}
                      value={fromAmountStr}
                      onChange={(e) => setFromAmountStr(e.target.value)}
                      currencyCode={fromCurrencyCode}
                      required
                      autoFocus
                    />
                  </div>
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="et-to-amount">{t('modals.editTransfer.toAmount')}</Label>
                    <CurrencyInput
                      id="et-to-amount"
                      type="number"
                      step={getMinorUnitsStep(toEvent.currencyMinorUnits)}
                      value={toAmountStr}
                      onChange={(e) => setToAmountStr(e.target.value)}
                      currencyCode={toCurrencyCode}
                      required
                    />
                  </div>
                </div>
                {fxRateDisplay !== null && (
                  <p className="text-xs text-muted-foreground">
                    {t('modals.editTransfer.fxRate', {
                      fromCurrency: fromCurrencyCode,
                      rate: fxRateDisplay,
                      toCurrency: toCurrencyCode,
                    })}
                  </p>
                )}
              </>
            ) : (
              <div className="flex flex-col gap-2">
                <Label htmlFor="et-amount">{t('modals.editTransfer.amount')}</Label>
                <CurrencyInput
                  id="et-amount"
                  type="number"
                  step={getMinorUnitsStep(fromEvent.currencyMinorUnits)}
                  value={fromAmountStr}
                  onChange={(e) => setFromAmountStr(e.target.value)}
                  currencyCode={fromCurrencyCode}
                  required
                  autoFocus
                />
              </div>
            )}

            {/* Note */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="et-note">{t('modals.editTransfer.note')}</Label>
              <Input id="et-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.editTransfer.notePlaceholder')} />
            </div>
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editTransfer.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.editTransfer.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditTransferModal
