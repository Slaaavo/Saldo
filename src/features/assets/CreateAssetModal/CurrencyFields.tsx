import { useTranslation } from 'react-i18next'
import CurrencySelect from '../../currency/CurrencySelect'
import type { Currency } from '../../../shared/types'
import { PINNED_CURRENCY_CODES } from '../../../shared/config/constants'
import { getMinorUnitsStep } from '../../../shared/utils/format'
import { CurrencyInput } from '../../../shared/ui/CurrencyInput'
import { DatePicker } from '../../../shared/ui/date-picker'
import { Input } from '../../../shared/ui/input'
import { Label } from '../../../shared/ui/label'
import { Button } from '../../../shared/ui/button'

interface CurrencyFieldsProps {
  currencies: Currency[]
  selectedCurrency: Currency | null
  setSelectedCurrency: (currency: Currency | null) => void
  initialValue: string
  setInitialValue: (value: string) => void
  selectedPerson: { personType: string } | null | undefined
  purchasePrice: string
  setPurchasePrice: (value: string) => void
  purchaseDate: string
  setPurchaseDate: (value: string) => void
  depreciationPeriod: string
  setDepreciationPeriod: (value: string) => void
}

const CurrencyFields = ({
  currencies,
  selectedCurrency,
  setSelectedCurrency,
  initialValue,
  setInitialValue,
  selectedPerson,
  purchasePrice,
  setPurchasePrice,
  purchaseDate,
  setPurchaseDate,
  depreciationPeriod,
  setDepreciationPeriod,
}: CurrencyFieldsProps) => {
  const { t } = useTranslation()

  return (
    <>
      <div className="flex flex-col gap-2">
        <Label htmlFor="create-asset-currency">{t('currency.label')}</Label>
        <CurrencySelect currencies={currencies} value={selectedCurrency} onChange={setSelectedCurrency} pinnedCurrencyCodes={PINNED_CURRENCY_CODES} />
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
      {selectedPerson?.personType === 'legal' && (
        <>
          <h3 className="text-base font-semibold border-t border-border pt-2 mt-1">{t('modals.assetDepreciation.sectionTitle')}</h3>
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-asset-purchase-price">{t('modals.assetDepreciation.purchasePrice')}</Label>
            <CurrencyInput
              id="create-asset-purchase-price"
              type="number"
              step={getMinorUnitsStep(selectedCurrency?.minorUnits ?? 2)}
              value={purchasePrice}
              onChange={(e) => setPurchasePrice(e.target.value)}
              placeholder="0"
              currencyCode={selectedCurrency?.code}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-asset-purchase-date">{t('modals.assetDepreciation.purchaseDate')}</Label>
            <DatePicker id="create-asset-purchase-date" value={purchaseDate} onChange={setPurchaseDate} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="create-asset-depreciation-period">{t('modals.assetDepreciation.depreciationPeriodMonths')}</Label>
            <Input id="create-asset-depreciation-period" type="number" min="0" value={depreciationPeriod} onChange={(e) => setDepreciationPeriod(e.target.value)} placeholder="" />
            <p className="text-xs text-muted-foreground">{t('modals.assetDepreciation.depreciationPeriodHelper')}</p>
            <div className="flex gap-2 flex-wrap">
              {(
                [
                  ['presetInstant', '0'],
                  ['preset4y', '48'],
                  ['preset5y', '60'],
                  ['preset20y', '240'],
                ] as const
              ).map(([key, val]) => (
                <Button key={key} type="button" variant="outline" size="sm" onClick={() => setDepreciationPeriod(val)}>
                  {t(`modals.assetDepreciation.${key}`)}
                </Button>
              ))}
            </div>
          </div>
        </>
      )}
    </>
  )
}

export default CurrencyFields
