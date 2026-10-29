import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import { CurrencyInput } from '@/shared/ui/CurrencyInput'
import { Label } from '@/shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select'
import { FORECAST_LINES, type TaxModelDetail } from '@/shared/types'
import { fromMinorUnits, getMinorUnitsStep } from '@/shared/utils/format'
import { monthName } from '../monthName'
import { EarlierProfitEditor } from './EarlierProfitEditor'
import { PERIODS, cellText, coversNoMonths } from './figuresDraft'
import { useTaxForecastFigures } from './useTaxForecastFigures'

interface FiguresFormProps {
  model: TaxModelDetail
  minorUnits: number
  currencyCode?: string
}

const MONTH_VALUES = Array.from({ length: 13 }, (_, month) => month)

export const FiguresForm = ({ model, minorUnits, currencyCode }: FiguresFormProps) => {
  const { t, i18n } = useTranslation()
  const { draft, saving, placeholder, setMonth, setPayrollMonthly, setCell, addEarlierProfit, changeEarlierProfit, removeEarlierProfit, handleSave, handleCancel } =
    useTaxForecastFigures(model, minorUnits)
  const amountStep = getMinorUnitsStep(minorUnits)

  return (
    <div className="px-4 md:px-10 py-8 max-w-3xl mx-auto space-y-6">
      <Card>
        <CardContent className="space-y-6 pt-6">
          <div className="space-y-2">
            <Label htmlFor="figures-actuals-through" className="text-sm block">
              {t('taxModels.figures.actualsThrough')}
            </Label>
            <Select value={String(draft.actualsThroughMonth)} onValueChange={(value) => setMonth(Number(value))}>
              <SelectTrigger id="figures-actuals-through">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {MONTH_VALUES.map((month) => (
                  <SelectItem key={month} value={String(month)}>
                    {month === 0 ? t('taxModels.figures.noMonthsYet') : monthName(month, i18n.language)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <p className="text-sm text-muted-foreground mb-3">{t('taxModels.figures.emptyHint')}</p>
            <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,11rem)_minmax(0,11rem)] items-center gap-x-3 gap-y-3">
              <span />
              {PERIODS.map((period) => (
                <span key={period} className="text-xs font-semibold text-muted-foreground">
                  {t(`taxModels.forecast.column.${period}`)}
                </span>
              ))}
              {FORECAST_LINES.map((line) => (
                <div key={line} className="contents">
                  <span className="text-sm">{t(`taxModels.forecast.line.${line}`)}</span>
                  {PERIODS.map((period) => {
                    const disabled = coversNoMonths(draft.actualsThroughMonth, period)
                    return (
                      <CurrencyInput
                        key={period}
                        type="number"
                        step={amountStep}
                        min="0"
                        currencyCode={currencyCode}
                        aria-label={`${t(`taxModels.forecast.line.${line}`)}, ${t(`taxModels.forecast.column.${period}`)}`}
                        disabled={disabled}
                        value={disabled ? '' : cellText(draft, line, period)}
                        onChange={(e) => setCell(line, period, e.target.value)}
                        placeholder={disabled ? undefined : placeholder(line, period)}
                      />
                    )
                  })}
                </div>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="figures-payroll-monthly" className="text-sm block">
              {t('taxModels.figures.payrollMonthly')}
            </Label>
            <CurrencyInput
              id="figures-payroll-monthly"
              type="number"
              step={amountStep}
              min="0"
              currencyCode={currencyCode}
              value={draft.payrollMonthly}
              onChange={(e) => setPayrollMonthly(e.target.value)}
              placeholder={fromMinorUnits(0, minorUnits)}
            />
          </div>
        </CardContent>
      </Card>

      {model.personType === 'legal' && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t('taxModels.figures.earlierProfitTitle')}</CardTitle>
          </CardHeader>
          <CardContent>
            <EarlierProfitEditor
              rows={draft.earlierProfits}
              onChange={changeEarlierProfit}
              onAdd={addEarlierProfit}
              onRemove={removeEarlierProfit}
              minorUnits={minorUnits}
              currencyCode={currencyCode}
            />
          </CardContent>
        </Card>
      )}

      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={handleCancel}>
          {t('taxModels.figures.cancel')}
        </Button>
        <Button type="button" onClick={handleSave} disabled={saving}>
          {t('taxModels.figures.save')}
        </Button>
      </div>
    </div>
  )
}
