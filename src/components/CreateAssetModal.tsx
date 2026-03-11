import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { PINNED_CURRENCY_CODES } from '../config/constants';
import { toMinorUnits, getMinorUnitsStep } from '../utils/format';
import type { Currency } from '../types';
import {
  listCurrencies,
  getConsolidationCurrency,
  listCustomUnits,
  createCustomUnit,
  createAccount,
} from '../api';
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
import CurrencySelect from './CurrencySelect';
import NumberValue from './NumberValue';
import { extractErrorMessage } from '../utils/errors';

interface Props {
  onSuccess: () => void;
  onClose: () => void;
}

type Denomination = 'currency' | 'unit';
const NEW_UNIT_VALUE = '__new__';

export default function CreateAssetModal({ onSuccess, onClose }: Props) {
  const { t } = useTranslation();

  const [denomination, setDenomination] = useState<Denomination>('currency');

  // Currency path state
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null);
  const [initialValue, setInitialValue] = useState('');

  // Shared asset name (used by both currency and unit paths)
  const [assetName, setAssetName] = useState('');

  // Unit path state
  const [customUnits, setCustomUnits] = useState<Currency[]>([]);
  const [selectedUnitId, setSelectedUnitId] = useState<string>('');
  const [newUnitName, setNewUnitName] = useState('');
  const [newUnitDecimals, setNewUnitDecimals] = useState('0');
  const [quantity, setQuantity] = useState('');
  const [pricePerUnit, setPricePerUnit] = useState('');
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null);

  const [submitting, setSubmitting] = useState(false);

  const isCreatingNewUnit = selectedUnitId === NEW_UNIT_VALUE;
  const selectedUnit = customUnits.find((u) => String(u.id) === selectedUnitId) ?? null;
  const effectiveUnit = isCreatingNewUnit ? null : selectedUnit;
  const effectiveMinorUnits = effectiveUnit?.minorUnits ?? (parseInt(newUnitDecimals) || 0);

  // Compute total for unit path
  const quantityMinor = quantity.trim() ? toMinorUnits(quantity, effectiveMinorUnits) : null;
  const priceDecimal = parseFloat(pricePerUnit);
  const totalMinor =
    quantityMinor !== null && !isNaN(priceDecimal) && priceDecimal > 0 && consolidationCurrency
      ? Math.round(
          (quantityMinor / Math.pow(10, effectiveMinorUnits)) *
            priceDecimal *
            Math.pow(10, consolidationCurrency.minorUnits),
        )
      : null;

  useEffect(() => {
    Promise.all([listCurrencies(false), getConsolidationCurrency(), listCustomUnits()])
      .then(([all, consolidation, units]) => {
        setCurrencies(all);
        setSelectedCurrency(consolidation);
        setConsolidationCurrency(consolidation);
        setCustomUnits(units);
        if (units.length > 0) {
          setSelectedUnitId(String(units[0].id));
        } else {
          setSelectedUnitId(NEW_UNIT_VALUE);
        }
      })
      .catch((err) => console.error('Failed to load data:', err));
  }, []);

  const handleSubmitCurrency = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!assetName.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.asset') }));
      return;
    }
    if (!selectedCurrency) {
      toast.error(t('validation.currencyRequired'));
      return;
    }
    let initialBalanceMinor: number | undefined;
    if (initialValue.trim()) {
      const parsed = parseFloat(initialValue);
      if (isNaN(parsed)) {
        toast.error(t('validation.invalidBalance'));
        return;
      }
      initialBalanceMinor = toMinorUnits(initialValue, selectedCurrency.minorUnits);
    }
    setSubmitting(true);
    try {
      await createAccount(assetName.trim(), selectedCurrency.id, initialBalanceMinor, 'asset');
      onSuccess();
    } catch (err) {
      toast.error(t('errors.createAccount', { error: extractErrorMessage(err) }));
    } finally {
      setSubmitting(false);
    }
  };

  const handleSubmitUnit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!assetName.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.asset') }));
      return;
    }
    if (!selectedUnitId) {
      toast.error(t('modals.createAsset.selectUnit'));
      return;
    }
    if (isCreatingNewUnit && !newUnitName.trim()) {
      toast.error(t('modals.createAsset.unitNameRequired'));
      return;
    }

    setSubmitting(true);
    try {
      let currencyId: number;
      let unitMinorUnits: number;

      if (isCreatingNewUnit) {
        const decimals = parseInt(newUnitDecimals) || 0;
        currencyId = await createCustomUnit(newUnitName.trim(), decimals);
        unitMinorUnits = decimals;
      } else {
        currencyId = effectiveUnit!.id;
        unitMinorUnits = effectiveUnit!.minorUnits;
      }

      let initialQuantityMinor: number | undefined;
      if (quantity.trim()) {
        const parsed = parseFloat(quantity);
        if (isNaN(parsed)) {
          toast.error(t('validation.invalidBalance'));
          setSubmitting(false);
          return;
        }
        initialQuantityMinor = toMinorUnits(quantity, unitMinorUnits);
      }

      const price = pricePerUnit.trim() || undefined;

      await createAccount(assetName.trim(), currencyId, initialQuantityMinor, 'asset', price);
      onSuccess();
    } catch (err) {
      toast.error(t('errors.createAccount', { error: extractErrorMessage(err) }));
    } finally {
      setSubmitting(false);
    }
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
          <DialogTitle>{t('modals.createAsset.title')}</DialogTitle>
        </DialogHeader>

        {/* Denomination toggle */}
        <div className="flex gap-2 px-6 pt-2">
          <button
            type="button"
            onClick={() => setDenomination('currency')}
            className={`flex-1 py-2 rounded-md text-sm font-medium border transition-colors ${
              denomination === 'currency'
                ? 'bg-primary text-primary-foreground border-primary'
                : 'bg-background text-muted-foreground border-border hover:bg-muted'
            }`}
          >
            {t('modals.createAsset.denominationCurrency')}
          </button>
          <button
            type="button"
            onClick={() => setDenomination('unit')}
            className={`flex-1 py-2 rounded-md text-sm font-medium border transition-colors ${
              denomination === 'unit'
                ? 'bg-primary text-primary-foreground border-primary'
                : 'bg-background text-muted-foreground border-border hover:bg-muted'
            }`}
          >
            {t('modals.createAsset.denominationUnit')}
          </button>
        </div>

        {denomination === 'currency' ? (
          <form
            onSubmit={handleSubmitCurrency}
            className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
          >
            <DialogBody className="flex flex-col gap-4">
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-asset-name">{t('modals.createAsset.nameLabel')}</Label>
                <Input
                  id="create-asset-name"
                  type="text"
                  value={assetName}
                  onChange={(e) => setAssetName(e.target.value)}
                  placeholder={t('modals.createAsset.namePlaceholder')}
                  required
                  autoFocus
                />
              </div>
              <div className="flex flex-col gap-2">
                <Label>{t('currency.label')}</Label>
                <CurrencySelect
                  currencies={currencies}
                  value={selectedCurrency}
                  onChange={setSelectedCurrency}
                  pinnedCurrencyCodes={PINNED_CURRENCY_CODES}
                />
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-asset-value">{t('modals.createAsset.initialValue')}</Label>
                <CurrencyInput
                  id="create-asset-value"
                  type="number"
                  step={getMinorUnitsStep(selectedCurrency?.minorUnits ?? 2)}
                  value={initialValue}
                  onChange={(e) => setInitialValue(e.target.value)}
                  placeholder="0"
                  currencyCode={selectedCurrency?.code}
                />
              </div>
            </DialogBody>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {t('modals.createAccount.cancel')}
              </Button>
              <Button type="submit" disabled={submitting}>
                {t('modals.createAccount.submit')}
              </Button>
            </DialogFooter>
          </form>
        ) : (
          <form
            onSubmit={handleSubmitUnit}
            className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
          >
            <DialogBody className="flex flex-col gap-4">
              {/* Asset name */}
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-asset-unit-name">{t('modals.createAsset.nameLabel')}</Label>
                <Input
                  id="create-asset-unit-name"
                  type="text"
                  value={assetName}
                  onChange={(e) => setAssetName(e.target.value)}
                  placeholder={t('modals.createAsset.namePlaceholder')}
                  autoFocus
                />
              </div>

              {/* Unit selector */}
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-asset-unit">{t('modals.createAsset.selectUnit')}</Label>
                <select
                  id="create-asset-unit"
                  value={selectedUnitId}
                  onChange={(e) => setSelectedUnitId(e.target.value)}
                  className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  {customUnits.map((u) => (
                    <option key={u.id} value={String(u.id)}>
                      {u.code}
                    </option>
                  ))}
                  <option value={NEW_UNIT_VALUE}>{t('modals.createAsset.createNewUnit')}</option>
                </select>
              </div>

              {/* New unit fields */}
              {isCreatingNewUnit && (
                <>
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="new-unit-name">{t('modals.createAsset.unitName')}</Label>
                    <Input
                      id="new-unit-name"
                      type="text"
                      value={newUnitName}
                      onChange={(e) => setNewUnitName(e.target.value)}
                      placeholder="e.g. VWCE"
                      autoFocus
                    />
                  </div>
                  <div className="flex flex-col gap-2">
                    <Label htmlFor="new-unit-decimals">
                      {t('modals.createAsset.decimalPlaces')}
                    </Label>
                    <Input
                      id="new-unit-decimals"
                      type="number"
                      min={0}
                      max={8}
                      value={newUnitDecimals}
                      onChange={(e) => setNewUnitDecimals(e.target.value)}
                    />
                  </div>
                </>
              )}

              {/* Initial quantity */}
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-asset-quantity">{t('modals.createAsset.quantity')}</Label>
                <CurrencyInput
                  id="create-asset-quantity"
                  type="number"
                  step={getMinorUnitsStep(effectiveMinorUnits)}
                  min={0}
                  value={quantity}
                  onChange={(e) => setQuantity(e.target.value)}
                  placeholder="0"
                  currencyCode={isCreatingNewUnit ? newUnitName || undefined : effectiveUnit?.code}
                />
              </div>

              {/* Price per unit */}
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-asset-price">{t('modals.createAsset.pricePerUnit')}</Label>
                <CurrencyInput
                  id="create-asset-price"
                  type="number"
                  step="0.01"
                  min={0}
                  value={pricePerUnit}
                  onChange={(e) => setPricePerUnit(e.target.value)}
                  placeholder="0"
                  currencyCode={consolidationCurrency?.code}
                />
              </div>

              {/* Computed total */}
              {totalMinor !== null && consolidationCurrency && (
                <div className="flex items-center justify-between py-2 px-3 rounded-md bg-muted text-sm">
                  <span className="text-muted-foreground">
                    {t('modals.createAsset.computedTotal')}
                  </span>
                  <NumberValue
                    value={totalMinor}
                    currencyCode={consolidationCurrency.code}
                    minorUnits={consolidationCurrency.minorUnits}
                    className="font-semibold"
                  />
                </div>
              )}
            </DialogBody>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {t('modals.createAccount.cancel')}
              </Button>
              <Button type="submit" disabled={submitting}>
                {t('modals.createAccount.submit')}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
