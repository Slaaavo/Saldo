import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import Decimal from 'decimal.js';
import { listFxRates } from '../api';
import { todayIso, toMinorUnits, fromMinorUnits, getMinorUnitsStep } from '../utils/format';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from './ui/dialog';
import { Button } from './ui/button';
import { CurrencyInput } from './CurrencyInput';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { DatePicker } from './ui/date-picker';
import NumberValue from './NumberValue';
import type { Currency } from '../types';

interface Props {
  accountId: number;
  accountName: string;
  currencyCode: string;
  currencyMinorUnits: number;
  balanceMinor: number;
  consolidationCurrency: Currency | null;
  onSubmit: (
    accountId: number,
    amountMinor: number | null,
    pricePerUnit: string | null,
    eventDate: string,
    note: string | null,
  ) => void;
  onClose: () => void;
}

function parsePriceInput(input: string): string | null {
  try {
    const d = new Decimal(input);
    if (d.isZero() || d.isNegative()) return null;
    return d.toString();
  } catch {
    return null;
  }
}

export default function UpdateAssetValueModal({
  accountId,
  accountName,
  currencyCode,
  currencyMinorUnits,
  balanceMinor,
  consolidationCurrency,
  onSubmit,
  onClose,
}: Props) {
  const { t } = useTranslation();

  const originalQuantityStr = fromMinorUnits(balanceMinor, currencyMinorUnits);
  const [quantity, setQuantity] = useState(originalQuantityStr);
  const [pricePerUnit, setPricePerUnit] = useState('');
  const [originalPriceStr, setOriginalPriceStr] = useState('');
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState('');

  useEffect(() => {
    listFxRates()
      .then((rates) => {
        // Find the latest rate for this custom unit
        const rate = rates.find((r) => r.toCurrencyCode === currencyCode);
        if (rate) {
          try {
            const price = new Decimal(1).div(
              new Decimal(`${rate.rateMantissa}e${rate.rateExponent}`),
            );
            const decimals = consolidationCurrency?.minorUnits ?? 2;
            const priceStr = price.toFixed(decimals);
            setPricePerUnit(priceStr);
            setOriginalPriceStr(priceStr);
          } catch {
            // ignore
          }
        }
      })
      .catch((err) => console.error('Failed to load FX rates:', err));
  }, [currencyCode, consolidationCurrency?.minorUnits]);

  const quantityMinor = quantity.trim() ? toMinorUnits(quantity, currencyMinorUnits) : null;
  const priceDecimal = parseFloat(pricePerUnit);
  const totalMinor =
    quantityMinor !== null && !isNaN(priceDecimal) && priceDecimal > 0 && consolidationCurrency
      ? Math.round(
          (quantityMinor / Math.pow(10, currencyMinorUnits)) *
            priceDecimal *
            Math.pow(10, consolidationCurrency.minorUnits),
        )
      : null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    const newQuantityMinor = quantity.trim() ? toMinorUnits(quantity, currencyMinorUnits) : null;
    const quantityChanged = newQuantityMinor !== null && newQuantityMinor !== balanceMinor;

    const priceInput = pricePerUnit.trim();
    const priceChanged = priceInput !== '' && priceInput !== originalPriceStr;
    const parsedPrice = priceInput ? parsePriceInput(priceInput) : null;

    if (priceChanged && !parsedPrice) {
      toast.error(t('units.invalidPrice'));
      return;
    }

    onSubmit(
      accountId,
      quantityChanged ? newQuantityMinor : null,
      priceChanged ? parsedPrice : null,
      date,
      note.trim() || null,
    );
  };

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {t('modals.updateAssetValue.title')} — {accountName}
          </DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
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
                step="0.01"
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
                <NumberValue
                  value={totalMinor}
                  currencyCode={consolidationCurrency.code}
                  minorUnits={consolidationCurrency.minorUnits}
                  className="font-semibold"
                />
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
              <Input
                id="update-asset-note"
                type="text"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t('modals.createBalanceUpdate.notePlaceholder')}
              />
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
  );
}
