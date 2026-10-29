import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { CurrencyInput } from '@/shared/ui/CurrencyInput'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { PercentageInput } from '@/shared/ui/PercentageInput'
import { fromMinorUnits, getMinorUnitsStep } from '@/shared/utils/format'
import type { EarlierProfitDraft } from './figuresDraft'

interface EarlierProfitEditorProps {
  rows: EarlierProfitDraft[]
  onChange: (index: number, patch: Partial<EarlierProfitDraft>) => void
  onAdd: () => void
  onRemove: (index: number) => void
  minorUnits: number
  currencyCode?: string
}

export const EarlierProfitEditor = ({ rows, onChange, onAdd, onRemove, minorUnits, currencyCode }: EarlierProfitEditorProps) => {
  const { t } = useTranslation()

  return (
    <div className="space-y-3">
      {rows.map((row, index) => {
        const rowLabel = (column: string) => `${t('taxModels.figures.earlierProfitRow', { row: index + 1 })}, ${t(column)}`
        return (
          <div key={index} className="grid grid-cols-[6rem_minmax(0,1fr)_8rem_auto] items-end gap-3">
            <div>
              <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.figures.year')}</Label>
              <Input
                type="number"
                inputMode="numeric"
                step="1"
                aria-label={rowLabel('taxModels.figures.year')}
                value={row.profitYear}
                onChange={(e) => onChange(index, { profitYear: e.target.value })}
                placeholder="2025"
              />
            </div>
            <div>
              <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.figures.amount')}</Label>
              <CurrencyInput
                type="number"
                step={getMinorUnitsStep(minorUnits)}
                min="0"
                currencyCode={currencyCode}
                aria-label={rowLabel('taxModels.figures.amount')}
                value={row.amount}
                onChange={(e) => onChange(index, { amount: e.target.value })}
                placeholder={fromMinorUnits(0, minorUnits)}
              />
            </div>
            <div>
              <Label className="text-xs text-muted-foreground mb-1 block">{t('taxModels.figures.dividendTax')}</Label>
              <PercentageInput
                type="number"
                step="0.01"
                min="0"
                max="100"
                aria-label={rowLabel('taxModels.figures.dividendTax')}
                value={row.dividendTaxPct}
                onChange={(e) => onChange(index, { dividendTaxPct: e.target.value })}
                placeholder="0.00"
              />
            </div>
            <Button type="button" variant="ghost" size="sm" className="text-destructive hover:text-destructive" onClick={() => onRemove(index)}>
              {t('taxModels.figures.remove')}
            </Button>
          </div>
        )
      })}
      <Button type="button" variant="outline" size="sm" onClick={onAdd}>
        {t('taxModels.figures.addYear')}
      </Button>
    </div>
  )
}
