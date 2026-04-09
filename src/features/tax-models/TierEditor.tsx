import { useTranslation } from 'react-i18next'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'

export interface TierFormState {
  thresholdMinor: string
  rateBps: string
}

interface TierEditorProps {
  tiers: TierFormState[]
  onTiersChange: (tiers: TierFormState[]) => void
}

const TierEditor = ({ tiers, onTiersChange }: TierEditorProps) => {
  const { t } = useTranslation()

  const handleTierChange = (index: number, field: keyof TierFormState, value: string) => {
    const updated = tiers.map((tier, i) => (i === index ? { ...tier, [field]: value } : tier))
    onTiersChange(updated)
  }

  const handleAddTier = () => {
    onTiersChange([...tiers, { thresholdMinor: '', rateBps: '' }])
  }

  const handleRemoveTier = (index: number) => {
    onTiersChange(tiers.filter((_, i) => i !== index))
  }

  return (
    <div className="space-y-2">
      {tiers.map((tier, index) => (
        <div key={index} className="flex gap-2 items-end">
          <div className="flex-1">
            <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.tierThreshold')}</Label>
            <CurrencyInput
              type="number"
              step="0.01"
              min="0"
              value={tier.thresholdMinor}
              onChange={(e) => handleTierChange(index, 'thresholdMinor', e.target.value)}
              placeholder="0.00"
            />
          </div>
          <div className="flex-1">
            <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.tierRate')}</Label>
            <PercentageInput
              type="number"
              step="0.01"
              min="0"
              max="100"
              value={tier.rateBps}
              onChange={(e) => handleTierChange(index, 'rateBps', e.target.value)}
              placeholder="0.00"
            />
          </div>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive shrink-0"
            disabled={tiers.length <= 1}
            onClick={() => handleRemoveTier(index)}
          >
            {t('taxModels.removeTier')}
          </Button>
        </div>
      ))}
      <Button type="button" variant="outline" size="sm" onClick={handleAddTier}>
        {t('taxModels.addTier')}
      </Button>
    </div>
  )
}

export default TierEditor
