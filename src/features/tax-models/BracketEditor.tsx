import { useTranslation } from 'react-i18next'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import { Label } from '../../shared/ui/label'
import { Card, CardContent, CardHeader, CardTitle } from '../../shared/ui/card'
import TierEditor from './TierEditor'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'
import type { TierFormState } from './TierEditor'

export type { TierFormState }

export interface BracketFormState {
  upperBoundMinor: string // '' means no upper bound (last/catch-all bracket)
  rateType: 'flat' | 'progressive'
  flatRateBps: string
  tiers: TierFormState[]
}

interface BracketEditorProps {
  brackets: BracketFormState[]
  onBracketsChange: (brackets: BracketFormState[]) => void
}

const emptyBracket = (): BracketFormState => ({ upperBoundMinor: '', rateType: 'flat', flatRateBps: '', tiers: [] })

const BracketEditor = ({ brackets, onBracketsChange }: BracketEditorProps) => {
  const { t } = useTranslation()

  const handleBracketChange = <K extends keyof BracketFormState>(index: number, field: K, value: BracketFormState[K]) => {
    const updated = brackets.map((b, i) => (i === index ? { ...b, [field]: value } : b))
    const firstEmptyIndex = updated.findIndex((b) => b.upperBoundMinor === '')
    if (firstEmptyIndex !== -1) {
      onBracketsChange(updated.slice(0, firstEmptyIndex + 1))
    } else {
      onBracketsChange([...updated, emptyBracket()])
    }
  }

  return (
    <div className="space-y-4">
      {brackets.map((bracket, index) => {
        const isLast = index === brackets.length - 1
        return (
          <Card key={index}>
            <CardHeader className="pb-2">
              <div className="flex justify-between items-center">
                <CardTitle className="text-base">
                  {t('taxModels.bracket')} {index + 1}
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label className="text-sm mb-1 block">{t('taxModels.bracketRateType')}</Label>
                <Select value={bracket.rateType} onValueChange={(val) => handleBracketChange(index, 'rateType', val as 'flat' | 'progressive')}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="flat">{t('taxModels.flatRate')}</SelectItem>
                    <SelectItem value="progressive">{t('taxModels.progressiveRate')}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              {bracket.rateType === 'flat' ? (
                <div>
                  <Label className="text-sm mb-1 block">{t('taxModels.flatRate')}</Label>
                  <PercentageInput
                    type="number"
                    step="0.01"
                    min="0"
                    max="100"
                    value={bracket.flatRateBps}
                    onChange={(e) => handleBracketChange(index, 'flatRateBps', e.target.value)}
                    placeholder="0.00"
                  />
                </div>
              ) : (
                <TierEditor
                  tiers={bracket.tiers.length > 0 ? bracket.tiers : [{ thresholdMinor: '', rateBps: '' }]}
                  onTiersChange={(tiers) => handleBracketChange(index, 'tiers', tiers)}
                />
              )}

              <div>
                <Label className="text-sm mb-1 block">{t('taxModels.bracketUpperBound')}</Label>
                <CurrencyInput
                  type="number"
                  step="0.01"
                  min="0"
                  value={bracket.upperBoundMinor}
                  onChange={(e) => handleBracketChange(index, 'upperBoundMinor', e.target.value)}
                  placeholder={isLast ? t('taxModels.bracketUpperBoundPlaceholder') : '0.00'}
                />
              </div>
            </CardContent>
          </Card>
        )
      })}
    </div>
  )
}

export default BracketEditor
