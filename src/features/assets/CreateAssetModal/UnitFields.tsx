import { useTranslation } from 'react-i18next'
import type { Currency } from '../../../shared/types'
import { getMinorUnitsStep } from '../../../shared/utils/format'
import { CurrencyInput } from '../../../shared/ui/CurrencyInput'
import { Input } from '../../../shared/ui/input'
import { Label } from '../../../shared/ui/label'
import NumberValue from '../../../shared/ui/NumberValue'
import { NEW_UNIT_VALUE } from './constants'

interface UnitFieldsProps {
  customUnits: Currency[]
  selectedUnitId: string
  setSelectedUnitId: (id: string) => void
  isCreatingNewUnit: boolean
  newUnitName: string
  setNewUnitName: (value: string) => void
  newUnitDecimals: string
  setNewUnitDecimals: (value: string) => void
  effectiveUnit: Currency | null
  effectiveMinorUnits: number
  quantity: string
  setQuantity: (value: string) => void
  pricePerUnit: string
  setPricePerUnit: (value: string) => void
  consolidationCurrency: Currency | null
  totalMinor: number | null
}

const UnitFields = ({
  customUnits,
  selectedUnitId,
  setSelectedUnitId,
  isCreatingNewUnit,
  newUnitName,
  setNewUnitName,
  newUnitDecimals,
  setNewUnitDecimals,
  effectiveUnit,
  effectiveMinorUnits,
  quantity,
  setQuantity,
  pricePerUnit,
  setPricePerUnit,
  consolidationCurrency,
  totalMinor,
}: UnitFieldsProps) => {
  const { t } = useTranslation()

  return (
    <>
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

      {isCreatingNewUnit && (
        <>
          <div className="flex flex-col gap-2">
            <Label htmlFor="new-unit-name">{t('modals.createAsset.unitName')}</Label>
            <Input id="new-unit-name" type="text" value={newUnitName} onChange={(e) => setNewUnitName(e.target.value)} placeholder="e.g. VWCE" autoFocus />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="new-unit-decimals">{t('modals.createAsset.decimalPlaces')}</Label>
            <Input id="new-unit-decimals" type="number" min={0} max={8} value={newUnitDecimals} onChange={(e) => setNewUnitDecimals(e.target.value)} />
          </div>
        </>
      )}

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

      <div className="flex flex-col gap-2">
        <Label htmlFor="create-asset-price">{t('modals.createAsset.pricePerUnit')}</Label>
        <CurrencyInput
          id="create-asset-price"
          type="number"
          step="any"
          min={0}
          value={pricePerUnit}
          onChange={(e) => setPricePerUnit(e.target.value)}
          placeholder="0"
          currencyCode={consolidationCurrency?.code}
        />
      </div>

      {totalMinor !== null && consolidationCurrency && (
        <div className="flex items-center justify-between py-2 px-3 rounded-md bg-muted text-sm">
          <span className="text-muted-foreground">{t('modals.createAsset.computedTotal')}</span>
          <NumberValue value={totalMinor} currencyCode={consolidationCurrency.code} minorUnits={consolidationCurrency.minorUnits} className="font-semibold" />
        </div>
      )}
    </>
  )
}

export default UnitFields
