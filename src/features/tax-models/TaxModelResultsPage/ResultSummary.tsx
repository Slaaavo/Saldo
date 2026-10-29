import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import NumberValue from '@/shared/ui/NumberValue'
import type { TaxCalculationResult } from '@/shared/types'
import { bpsToPct } from '@/shared/utils/format'

interface ResultSummaryProps {
  result: TaxCalculationResult
  calendarYear: number
  minorUnits: number
  currencyCode?: string
}

interface SummaryRowProps {
  label: string
  valueMinor: number
  minorUnits: number
  currencyCode?: string
  sub?: { label: string; valueMinor?: number }
}

const SummaryRow = ({ label, valueMinor, minorUnits, currencyCode, sub }: SummaryRowProps) => (
  <div className="py-2">
    <div className="flex justify-between">
      <span>{label}</span>
      <NumberValue value={valueMinor} minorUnits={minorUnits} currencyCode={currencyCode} />
    </div>
    {sub && (
      <div className="flex justify-between text-xs text-muted-foreground">
        <span>{sub.label}</span>
        {sub.valueMinor !== undefined && <NumberValue value={sub.valueMinor} minorUnits={minorUnits} currencyCode={currencyCode} />}
      </div>
    )}
  </div>
)

export const ResultSummary = ({ result, calendarYear, minorUnits, currencyCode }: ResultSummaryProps) => {
  const { t } = useTranslation()
  const isLegal = result.personType === 'legal'
  const shows = { minorUnits, currencyCode }
  const taxLabel =
    isLegal && result.minimumTaxMinor === null && result.appliedFlatRateBps !== null
      ? t('taxModels.forecast.corporateTaxAt', { rate: `${bpsToPct(result.appliedFlatRateBps)}%` })
      : t('taxModels.totalTax')

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{t('taxModels.resultsTitle')}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="text-sm divide-y divide-border">
          <SummaryRow label={t('taxModels.totalIncome')} valueMinor={result.totalIncomeMinor} {...shows} />
          <SummaryRow label={t('taxModels.deductibleExpenses')} valueMinor={result.totalTaxDeductibleExpensesMinor} {...shows} />
          <SummaryRow label={t('taxModels.grossProfit')} valueMinor={result.taxBasisMinor} {...shows} />
          <SummaryRow
            label={taxLabel}
            valueMinor={result.taxAmountMinor}
            sub={result.minimumTaxMinor !== null ? { label: t('taxModels.forecast.minimumTaxApplies'), valueMinor: result.computedTaxMinor } : undefined}
            {...shows}
          />
          <SummaryRow label={t('taxModels.nonDeductibleExpenses')} valueMinor={result.totalNonTaxDeductibleExpensesMinor} {...shows} />
          <SummaryRow label={t('taxModels.netProfit')} valueMinor={result.totalProfitMinor} {...shows} />
          {isLegal && (
            <>
              <SummaryRow label={t('taxModels.reserveFundAllocation')} valueMinor={result.reserveFundGenerationMinor} {...shows} />
              <SummaryRow
                label={t('taxModels.forecast.dividendFromYear', { year: calendarYear })}
                valueMinor={result.dividendFromYearMinor}
                sub={{ label: t('taxModels.forecast.dividendTax'), valueMinor: result.dividendFromYearTaxMinor }}
                {...shows}
              />
              {result.earlierProfits.map((row) => (
                <SummaryRow
                  key={row.profitYear}
                  label={t('taxModels.forecast.profitLeftFrom', { year: row.profitYear })}
                  valueMinor={row.payableMinor}
                  sub={{ label: t('taxModels.forecast.dividendTaxAt', { rate: `${bpsToPct(row.dividendTaxRateBps)}%` }), valueMinor: row.dividendTaxMinor }}
                  {...shows}
                />
              ))}
              <SummaryRow label={t('taxModels.forecast.maximumDividend')} valueMinor={result.dividendMinor} {...shows} />
              <SummaryRow label={t('taxModels.withholdingTax')} valueMinor={result.withholdingTaxMinor} {...shows} />
              <SummaryRow label={t('taxModels.netDividend')} valueMinor={result.netDividendMinor} {...shows} />
            </>
          )}
          <SummaryRow label={t('taxModels.monthlyTaxBurden')} valueMinor={result.monthlyTaxBurdenMinor} {...shows} />
        </div>
      </CardContent>
    </Card>
  )
}
