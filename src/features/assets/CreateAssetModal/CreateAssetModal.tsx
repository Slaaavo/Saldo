import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQuery } from '@tanstack/react-query'
import { toMinorUnits, todayIso } from '../../../shared/utils/format'
import type { Currency } from '../../../shared/types'
import { listCurrencies, getConsolidationCurrency, listCustomUnits, createCustomUnit, createAccount, listPersons } from '../../../shared/api'
import { useSelectedPerson } from '../../../app/useSelectedPerson'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../../shared/ui/dialog'
import { Button } from '../../../shared/ui/button'
import { Input } from '../../../shared/ui/input'
import { Label } from '../../../shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../shared/ui/select'
import { extractErrorMessage } from '../../../shared/utils/errors'
import CurrencyFields from './CurrencyFields'
import UnitFields from './UnitFields'
import { NEW_UNIT_VALUE } from './constants'

interface Props {
  onSuccess: () => void
  onClose: () => void
}

type Denomination = 'currency' | 'unit'

const CreateAssetModal = ({ onSuccess, onClose }: Props) => {
  const { t } = useTranslation()
  const { selectedPersonId } = useSelectedPerson()
  const { data: persons } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  // undefined = auto-derive from default person; number = explicitly chosen
  const [personIdOverride, setPersonIdOverride] = useState<number | undefined>(selectedPersonId ?? undefined)
  const personId: number | null = personIdOverride !== undefined ? personIdOverride : (persons?.find((p) => p.isDefault)?.id ?? null)
  const selectedPerson = persons?.find((p) => p.id === personId)

  const [denomination, setDenomination] = useState<Denomination>('currency')

  // Currency path state
  const [currencies, setCurrencies] = useState<Currency[]>([])
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null)
  const [initialValue, setInitialValue] = useState('')

  // Shared asset name (used by both currency and unit paths)
  const [assetName, setAssetName] = useState('')

  // Unit path state
  const [customUnits, setCustomUnits] = useState<Currency[]>([])
  const [selectedUnitId, setSelectedUnitId] = useState<string>('')
  const [newUnitName, setNewUnitName] = useState('')
  const [newUnitDecimals, setNewUnitDecimals] = useState('0')
  const [quantity, setQuantity] = useState('')
  const [pricePerUnit, setPricePerUnit] = useState('')
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null)

  const [submitting, setSubmitting] = useState(false)

  // Depreciation state (currency path only)
  const [purchasePrice, setPurchasePrice] = useState('')
  const [purchaseDate, setPurchaseDate] = useState(todayIso().slice(0, 10))
  const [depreciationPeriod, setDepreciationPeriod] = useState('')

  const isCreatingNewUnit = selectedUnitId === NEW_UNIT_VALUE
  const selectedUnit = customUnits.find((u) => String(u.id) === selectedUnitId) ?? null
  const effectiveUnit = isCreatingNewUnit ? null : selectedUnit
  const effectiveMinorUnits = effectiveUnit?.minorUnits ?? (parseInt(newUnitDecimals) || 0)

  // Compute total for unit path
  const quantityMinor = quantity.trim() ? toMinorUnits(quantity, effectiveMinorUnits) : null
  const priceDecimal = parseFloat(pricePerUnit)
  const totalMinor =
    quantityMinor !== null && !isNaN(priceDecimal) && priceDecimal > 0 && consolidationCurrency
      ? Math.round((quantityMinor / Math.pow(10, effectiveMinorUnits)) * priceDecimal * Math.pow(10, consolidationCurrency.minorUnits))
      : null

  useEffect(() => {
    Promise.all([listCurrencies(false), getConsolidationCurrency(), listCustomUnits()])
      .then(([all, consolidation, units]) => {
        setCurrencies(all)
        setSelectedCurrency(consolidation)
        setConsolidationCurrency(consolidation)
        setCustomUnits(units)
        if (units.length > 0) {
          setSelectedUnitId(String(units[0].id))
        } else {
          setSelectedUnitId(NEW_UNIT_VALUE)
        }
      })
      .catch((err) => console.error('Failed to load data:', err))
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!assetName.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.asset') }))
      return
    }

    setSubmitting(true)
    try {
      if (denomination === 'currency') {
        // Currency path validation
        if (!selectedCurrency) {
          toast.error(t('validation.currencyRequired'))
          return
        }

        let initialBalanceMinor: number | undefined
        if (initialValue.trim()) {
          const parsed = parseFloat(initialValue)
          if (isNaN(parsed)) {
            toast.error(t('validation.invalidBalance'))
            return
          }
          initialBalanceMinor = toMinorUnits(initialValue, selectedCurrency.minorUnits)
        }

        const purchasePriceMinor = purchasePrice.trim() ? toMinorUnits(purchasePrice, selectedCurrency.minorUnits) : null
        const purchaseDateValue = purchaseDate.trim() ? purchaseDate.trim() : null
        const periodMonths = depreciationPeriod.trim() !== '' ? Math.max(0, parseInt(depreciationPeriod, 10)) : null

        await createAccount(
          assetName.trim(),
          selectedCurrency.id,
          initialBalanceMinor,
          'asset',
          undefined,
          undefined,
          undefined,
          personId ?? undefined,
          purchasePriceMinor,
          purchaseDateValue,
          periodMonths,
        )
      } else {
        // Unit path validation
        if (!selectedUnitId) {
          toast.error(t('modals.createAsset.selectUnit'))
          return
        }
        if (isCreatingNewUnit && !newUnitName.trim()) {
          toast.error(t('modals.createAsset.unitNameRequired'))
          return
        }

        let currencyId: number
        let unitMinorUnits: number

        if (isCreatingNewUnit) {
          const decimals = parseInt(newUnitDecimals) || 0
          currencyId = await createCustomUnit(newUnitName.trim(), decimals)
          unitMinorUnits = decimals
        } else {
          currencyId = effectiveUnit!.id
          unitMinorUnits = effectiveUnit!.minorUnits
        }

        let initialQuantityMinor: number | undefined
        if (quantity.trim()) {
          const parsed = parseFloat(quantity)
          if (isNaN(parsed)) {
            toast.error(t('validation.invalidBalance'))
            return
          }
          initialQuantityMinor = toMinorUnits(quantity, unitMinorUnits)
        }

        const price = pricePerUnit.trim() || undefined

        await createAccount(assetName.trim(), currencyId, initialQuantityMinor, 'asset', price, undefined, undefined, personId ?? undefined)
      }
      onSuccess()
    } catch (err) {
      toast.error(t('errors.createAccount', { error: extractErrorMessage(err) }))
    } finally {
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
          <DialogTitle>{t('modals.createAsset.title')}</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0">
          {/* Denomination toggle */}
          <div className="flex gap-2 px-6 pt-2">
            <button
              type="button"
              onClick={() => setDenomination('currency')}
              className={`flex-1 py-2 rounded-md text-sm font-medium border transition-colors ${
                denomination === 'currency' ? 'bg-primary text-primary-foreground border-primary' : 'bg-background text-muted-foreground border-border hover:bg-muted'
              }`}
            >
              {t('modals.createAsset.denominationCurrency')}
            </button>
            <button
              type="button"
              onClick={() => setDenomination('unit')}
              className={`flex-1 py-2 rounded-md text-sm font-medium border transition-colors ${
                denomination === 'unit' ? 'bg-primary text-primary-foreground border-primary' : 'bg-background text-muted-foreground border-border hover:bg-muted'
              }`}
            >
              {t('modals.createAsset.denominationUnit')}
            </button>
          </div>

          <DialogBody className="flex flex-col gap-4">
            {/* Shared: Asset name */}
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

            {/* Shared: Person selector */}
            {persons && persons.length > 1 && (
              <div className="flex flex-col gap-2">
                <Label>{t('persons.selector')}</Label>
                <Select value={personId === null ? 'none' : String(personId)} onValueChange={(val) => setPersonIdOverride(Number(val))}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {persons.map((p) => (
                      <SelectItem key={p.id} value={String(p.id)}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            {/* Denomination-specific fields */}
            {denomination === 'currency' ? (
              <CurrencyFields
                currencies={currencies}
                selectedCurrency={selectedCurrency}
                setSelectedCurrency={setSelectedCurrency}
                initialValue={initialValue}
                setInitialValue={setInitialValue}
                selectedPerson={selectedPerson}
                purchasePrice={purchasePrice}
                setPurchasePrice={setPurchasePrice}
                purchaseDate={purchaseDate}
                setPurchaseDate={setPurchaseDate}
                depreciationPeriod={depreciationPeriod}
                setDepreciationPeriod={setDepreciationPeriod}
              />
            ) : (
              <UnitFields
                customUnits={customUnits}
                selectedUnitId={selectedUnitId}
                setSelectedUnitId={setSelectedUnitId}
                isCreatingNewUnit={isCreatingNewUnit}
                newUnitName={newUnitName}
                setNewUnitName={setNewUnitName}
                newUnitDecimals={newUnitDecimals}
                setNewUnitDecimals={setNewUnitDecimals}
                effectiveUnit={effectiveUnit}
                effectiveMinorUnits={effectiveMinorUnits}
                quantity={quantity}
                setQuantity={setQuantity}
                pricePerUnit={pricePerUnit}
                setPricePerUnit={setPricePerUnit}
                consolidationCurrency={consolidationCurrency}
                totalMinor={totalMinor}
              />
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
      </DialogContent>
    </Dialog>
  )
}

export default CreateAssetModal
