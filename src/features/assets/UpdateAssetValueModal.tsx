import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import Decimal from 'decimal.js'
import { listFxRates } from '../../shared/api'
import { todayIso, toMinorUnits, fromMinorUnits, getMinorUnitsStep } from '../../shared/utils/format'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import NumberValue from '../../shared/ui/NumberValue'
import type { Currency } from '../../shared/types'
import { extractErrorMessage } from '../../shared/utils/errors'
import { computeUnitTotalMinor, parsePositivePrice } from './unitPricing'

interface Props {
  accountId: number
  accountName: string
  currencyCode: string
  currencyMinorUnits: number
  balanceMinor: number
  consolidationCurrency: Currency | null
  onSubmit: (accountId: number, amountMinor: number | null, pricePerUnit: string | null, eventDate: string, note: string | null) => void
  onClose: () => void
}

const UpdateAssetValueModal = ({ accountId, accountName, currencyCode, currencyMinorUnits, balanceMinor, consolidationCurrency, onSubmit, onClose }: Props) => {
  const { t } = useTranslation()

  const originalQuantityStr = fromMinorUnits(balanceMinor, currencyMinorUnits)
  const [quantity, setQuantity] = useState(originalQuantityStr)
  const [pricePerUnit, setPricePerUnit] = useState('')
  const [originalPriceStr, setOriginalPriceStr] = useState('')
  const [originalPriceDate, setOriginalPriceDate] = useState('')
  const [date, setDate] = useState(todayIso())
  const [note, setNote] = useState('')

  const consolidationCurrencyCode = consolidationCurrency?.code

  useEffect(() => {
    if (!consolidationCurrencyCode) return
    listFxRates()
      .then((rates) => {
        // Latest price of this unit in the current consolidation currency; rates quoted in an earlier one no longer apply
        const rate = rates.find((r) => r.fromCurrencyCode === consolidationCurrencyCode && r.toCurrencyCode === currencyCode)
        if (rate) {
          try {
            const raw = new Decimal(`${rate.rateMantissa}e${rate.rateExponent}`)
            const price = rate.isDirect ? raw : new Decimal(1).div(raw)
            const priceStr = price.toDecimalPlaces(8).toString()
            setPricePerUnit(priceStr)
            setOriginalPriceStr(priceStr)
            setOriginalPriceDate(rate.date)
          } catch {
            // ignore
          }
        }
      })
      .catch((err) => toast.error(extractErrorMessage(err)))
  }, [currencyCode, consolidationCurrencyCode])

  const quantityMinor = quantity.trim() ? toMinorUnits(quantity, currencyMinorUnits) : null
  const totalMinor =
    quantityMinor !== null && consolidationCurrency ? computeUnitTotalMinor(quantityMinor, currencyMinorUnits, pricePerUnit, consolidationCurrency.minorUnits) : null

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    const newQuantityMinor = quantity.trim() ? toMinorUnits(quantity, currencyMinorUnits) : null
    const quantityChanged = newQuantityMinor !== null && newQuantityMinor !== balanceMinor

    const priceInput = pricePerUnit.trim()
    // A new date records the price for that date even when the price itself is unchanged
    const priceChanged = priceInput !== '' && (priceInput !== originalPriceStr || date !== originalPriceDate)
    const parsedPrice = priceInput ? parsePositivePrice(priceInput) : null

    if (priceChanged && !parsedPrice) {
      toast.error(t('units.invalidPrice'))
      return
    }

    onSubmit(accountId, quantityChanged ? newQuantityMinor : null, priceChanged ? parsedPrice : null, date, note.trim() || null)
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
          <DialogTitle>
            {t('modals.updateAssetValue.title')} — {accountName}
          </DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            {/* Quantity */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="update-asset-quantity">
                {t('modals.updateAssetValue.quantity')} ({currencyCode})
              </Label>
              <CurrencyInput
                id="update-asset-quantity"
                type="number"
                step={getMinorUnitsStep(currencyMinorUnits)}
                min={0}
                value={quantity}
                onChange={(e) => setQuantity(e.target.value)}
                currencyCode={currencyCode}
              />
            </div>

            {/* Price per unit */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="update-asset-price">
                {t('modals.updateAssetValue.pricePerUnit')} ({consolidationCurrency?.code})
              </Label>
              <CurrencyInput
                id="update-asset-price"
                type="number"
                step="any"
                min={0}
                value={pricePerUnit}
                onChange={(e) => setPricePerUnit(e.target.value)}
                currencyCode={consolidationCurrency?.code}
              />
            </div>

            {/* Computed total */}
            {totalMinor !== null && consolidationCurrency && (
              <div className="flex items-center justify-between py-2 px-3 rounded-md bg-muted text-sm">
                <span className="text-muted-foreground">{t('modals.updateAssetValue.total')}</span>
                <NumberValue value={totalMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} className="font-semibold" />
              </div>
            )}

            {/* Date */}
            <div className="flex flex-col gap-2">
              <Label>{t('modals.createBalanceUpdate.date')}</Label>
              <DatePicker value={date} onChange={setDate} />
            </div>

            {/* Note */}
            <div className="flex flex-col gap-2">
              <Label htmlFor="update-asset-note">{t('modals.createBalanceUpdate.note')}</Label>
              <Input id="update-asset-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.createBalanceUpdate.notePlaceholder')} />
            </div>
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createAccount.cancel')}
            </Button>
            <Button type="submit">{t('modals.editBalanceUpdate.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default UpdateAssetValueModal
