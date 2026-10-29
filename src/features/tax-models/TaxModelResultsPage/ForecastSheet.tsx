import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import NumberValue from '@/shared/ui/NumberValue'
import type { ForecastCell, TaxCalculationResult } from '@/shared/types'
import { monthName } from '../monthName'

interface ForecastSheetProps {
  result: TaxCalculationResult
  minorUnits: number
  currencyCode?: string
}

const periodsHeading = (t: TFunction, language: string, actualsThroughMonth: number): string => {
  if (actualsThroughMonth >= 12) return t('taxModels.forecast.wholeYearActual')
  if (actualsThroughMonth <= 0) return t('taxModels.forecast.wholeYearProjected')
  return t('taxModels.forecast.actualAndProjected', {
    from: monthName(1, language),
    to: monthName(actualsThroughMonth, language),
    restFrom: monthName(actualsThroughMonth + 1, language),
    restTo: monthName(12, language),
  })
}

interface SheetCellProps {
  cell: ForecastCell
  minorUnits: number
  currencyCode?: string
}

const SheetCell = ({ cell, minorUnits, currencyCode }: SheetCellProps) => {
  const { t } = useTranslation()
  return (
    <td className="py-2 pl-3 text-right align-top">
      <NumberValue value={cell.amountMinor} minorUnits={minorUnits} currencyCode={currencyCode} />
      <div className="text-xs text-muted-foreground">{t(`taxModels.forecast.source.${cell.source}`)}</div>
    </td>
  )
}

export const ForecastSheet = ({ result, minorUnits, currencyCode }: ForecastSheetProps) => {
  const { t, i18n } = useTranslation()

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{periodsHeading(t, i18n.language, result.actualsThroughMonth)}</CardTitle>
      </CardHeader>
      <CardContent>
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border text-xs text-muted-foreground">
              <th />
              <th className="py-2 pl-3 text-right font-semibold">{t('taxModels.forecast.column.soFar')}</th>
              <th className="py-2 pl-3 text-right font-semibold">{t('taxModels.forecast.column.rest')}</th>
              <th className="py-2 pl-3 text-right font-semibold">{t('taxModels.forecast.column.year')}</th>
            </tr>
          </thead>
          <tbody>
            {result.lines.map((row) => (
              <tr key={row.line} className="border-b border-border last:border-b-0">
                <th scope="row" className="py-2 text-left font-normal align-top">
                  {t(`taxModels.forecast.line.${row.line}`)}
                </th>
                <SheetCell cell={row.soFar} minorUnits={minorUnits} currencyCode={currencyCode} />
                <SheetCell cell={row.rest} minorUnits={minorUnits} currencyCode={currencyCode} />
                <td className="py-2 pl-3 text-right align-top">
                  <NumberValue value={row.yearMinor} minorUnits={minorUnits} currencyCode={currencyCode} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </CardContent>
    </Card>
  )
}
