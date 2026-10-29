import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { CurrencyInput } from '@/shared/ui/CurrencyInput'
import { Label } from '@/shared/ui/label'
import { fromMinorUnits, getMinorUnitsStep } from '@/shared/utils/format'
import { emptyMinimumTaxBand, type MinimumTaxBandFormState } from './minimumTaxBands'

interface MinimumTaxEditorProps {
  bands: MinimumTaxBandFormState[]
  onBandsChange: (bands: MinimumTaxBandFormState[]) => void
  minorUnits: number
  currencyCode?: string
}

const MinimumTaxEditor = ({ bands, onBandsChange, minorUnits, currencyCode }: MinimumTaxEditorProps) => {
  const { t } = useTranslation()
  const step = getMinorUnitsStep(minorUnits)

  const handleBandChange = <K extends keyof MinimumTaxBandFormState>(index: number, field: K, value: MinimumTaxBandFormState[K]) => {
    const updated = bands.map((band, i) => (i === index ? { ...band, [field]: value } : band))
    if (updated[updated.length - 1].upperBoundMinor !== '') {
      updated.push(emptyMinimumTaxBand())
    }
    onBandsChange(updated)
  }

  const handleRemoveBand = (index: number) => {
    const remaining = bands.filter((_, i) => i !== index)
    onBandsChange(remaining.map((band, i) => (i === remaining.length - 1 ? { ...band, upperBoundMinor: '' } : band)))
  }

  return (
    <div className="space-y-3">
      {bands.map((band, index) => {
        const isLast = index === bands.length - 1
        const bandLabel = (column: string) => `${t('taxModels.minimumTaxBandRow', { band: index + 1 })}, ${t(column)}`
        return (
          <div key={index} className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-end gap-3">
            <div>
              <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.minimumTaxUpTo')}</Label>
              <CurrencyInput
                type="number"
                step={step}
                min="0"
                currencyCode={currencyCode}
                aria-label={bandLabel('taxModels.minimumTaxUpTo')}
                value={band.upperBoundMinor}
                onChange={(e) => handleBandChange(index, 'upperBoundMinor', e.target.value)}
                placeholder={isLast ? t('taxModels.bracketUpperBoundPlaceholder') : fromMinorUnits(0, minorUnits)}
              />
            </div>
            <div>
              <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.minimumTaxAmount')}</Label>
              <CurrencyInput
                type="number"
                step={step}
                min="0"
                currencyCode={currencyCode}
                aria-label={bandLabel('taxModels.minimumTaxAmount')}
                value={band.amountMinor}
                onChange={(e) => handleBandChange(index, 'amountMinor', e.target.value)}
                placeholder={fromMinorUnits(0, minorUnits)}
              />
            </div>
            <Button type="button" variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => handleRemoveBand(index)}>
              {t('taxModels.removeMinimumTaxBand')}
            </Button>
          </div>
        )
      })}
      {bands.length === 0 && (
        <Button type="button" variant="outline" size="sm" onClick={() => onBandsChange([emptyMinimumTaxBand()])}>
          {t('taxModels.addMinimumTaxBand')}
        </Button>
      )}
    </div>
  )
}

export default MinimumTaxEditor
