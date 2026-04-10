import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from '@tanstack/react-router'
import { Button } from '../../shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '../../shared/ui/card'
import NumberValue from '../../shared/ui/NumberValue'
import { getTaxModel, calculateTaxModel } from '../../shared/api'
import { usePageTitle } from '../../app/usePageHeader'
import type { TaxEventBreakdown } from '../../shared/types'

const TaxModelResultsPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const params = useParams({ strict: false }) as { modelId?: string }
  const modelId = params.modelId

  const { data: model, isLoading } = useQuery({
    queryKey: ['taxModel', Number(modelId)],
    queryFn: () => getTaxModel(Number(modelId)),
    enabled: !!modelId,
  })

  const { data: result, isLoading: isResultLoading } = useQuery({
    queryKey: ['taxModelResults', Number(modelId)],
    queryFn: () => calculateTaxModel(Number(modelId)),
    enabled: !!modelId,
  })

  usePageTitle(model != null ? `${model.name} – ${model.calendarYear}` : t('taxModels.resultsTitle'))

  if (isLoading || isResultLoading) return null

  const isLegal = result?.personType === 'legal'
  const hasNoEvents = result != null && result.eventBreakdowns.length === 0

  const breakdowns = result?.eventBreakdowns ?? []
  const revenueRows = breakdowns.filter((e) => e.eventType === 'revenue')
  const expenseRows = breakdowns.filter((e) => e.eventType === 'expense')

  const sumGroup = (rows: TaxEventBreakdown[]) => ({
    amountMinor: rows.reduce((s, r) => s + r.amountMinor, 0),
    netAmountMinor: rows.reduce((s, r) => s + r.netAmountMinor, 0),
    vatAmountMinor: rows.reduce((s, r) => s + r.vatAmountMinor, 0),
    reclaimableVatMinor: rows.reduce((s, r) => s + r.reclaimableVatMinor, 0),
    taxDeductibleCostMinor: rows.reduce((s, r) => s + r.taxDeductibleCostMinor, 0),
    nonTaxDeductibleCostMinor: rows.reduce((s, r) => s + r.nonTaxDeductibleCostMinor, 0),
  })

  const revenueTotals = sumGroup(revenueRows)
  const expenseTotals = sumGroup(expenseRows)

  return (
    <div className="px-4 md:px-10 py-8 max-w-2xl mx-auto">
      <div className="flex items-center gap-4 mb-4">
        <Button variant="ghost" size="sm" onClick={() => navigate({ to: '/tax-models' })}>
          ← {t('taxModels.backToList')}
        </Button>
        <Button variant="outline" size="sm" className="ml-auto" onClick={() => navigate({ to: '/tax-models/$modelId/edit', params: { modelId: modelId! } })}>
          {t('taxModels.adjustSettings')}
        </Button>
      </div>

      {hasNoEvents ? (
        <p className="text-sm text-muted-foreground mt-6">{t('taxModels.noEventsForYear')}</p>
      ) : (
        <>
          <Card>
            <CardHeader>
              <CardTitle className="text-base">{t('taxModels.resultsTitle')}</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-sm">
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.totalIncome')}</span>
                  <NumberValue value={result?.totalIncomeMinor ?? 0} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.deductibleExpenses')}</span>
                  <NumberValue value={result?.totalTaxDeductibleExpensesMinor ?? 0} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.grossProfit')}</span>
                  <NumberValue value={result?.taxBasisMinor ?? 0} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.totalTax')}</span>
                  <NumberValue value={result?.taxAmountMinor ?? 0} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.nonDeductibleExpenses')}</span>
                  <NumberValue value={result?.totalNonTaxDeductibleExpensesMinor ?? 0} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.netProfit')}</span>
                  <NumberValue value={result?.totalProfitMinor ?? 0} minorUnits={2} />
                </div>
                {isLegal && (
                  <>
                    <div className="flex justify-between py-2 border-b border-border">
                      <span>{t('taxModels.reserveFundAllocation')}</span>
                      <NumberValue value={result?.reserveFundGenerationMinor ?? 0} minorUnits={2} />
                    </div>
                    <div className="flex justify-between py-2 border-b border-border">
                      <span>{t('taxModels.dividendAmount')}</span>
                      <NumberValue value={result?.dividendMinor ?? 0} minorUnits={2} />
                    </div>
                    <div className="flex justify-between py-2 border-b border-border">
                      <span>{t('taxModels.withholdingTax')}</span>
                      <NumberValue value={result?.withholdingTaxMinor ?? 0} minorUnits={2} />
                    </div>
                    <div className="flex justify-between py-2 border-b border-border">
                      <span>{t('taxModels.netDividend')}</span>
                      <NumberValue value={result?.netDividendMinor ?? 0} minorUnits={2} />
                    </div>
                  </>
                )}
                <div className="flex justify-between py-2 border-border">
                  <span>{t('taxModels.monthlyTaxBurden')}</span>
                  <NumberValue value={result?.monthlyTaxBurdenMinor ?? 0} minorUnits={2} />
                </div>
              </div>
            </CardContent>
          </Card>

          <details className="mt-6 text-sm">
            <summary className="cursor-pointer select-none text-muted-foreground hover:text-foreground py-2">{t('taxModels.breakdownSection')}</summary>
            <div className="mt-3 overflow-x-auto">
              <table className="w-full text-xs border-collapse whitespace-nowrap">
                <thead>
                  <tr className="border-b border-border">
                    <th className="text-left py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownDate')}</th>
                    <th className="text-left py-2 pr-3 font-semibold text-muted-foreground">{t('taxModels.breakdownNote')}</th>
                    <th className="text-left py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownType')}</th>
                    <th className="text-right py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownAmount')}</th>
                    <th className="text-right py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownNetAmount')}</th>
                    <th className="text-right py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownVat')}</th>
                    <th className="text-right py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownReclaimableVat')}</th>
                    <th className="text-right py-2 pr-3 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownDeductibleCost')}</th>
                    <th className="text-right py-2 font-semibold text-muted-foreground whitespace-nowrap">{t('taxModels.breakdownNonDeductibleCost')}</th>
                  </tr>
                </thead>
                <tbody>
                  {revenueRows.length > 0 && (
                    <>
                      <tr className="bg-muted/50">
                        <td colSpan={9} className="py-1 px-1 font-semibold text-foreground">
                          {t('taxModels.revenueGroup')}
                        </td>
                      </tr>
                      {revenueRows.map((row) => (
                        <tr key={row.eventId} className="border-b border-border/40">
                          <td className="py-1.5 pr-3 whitespace-nowrap">{row.eventDate.slice(0, 10)}</td>
                          <td className="py-1.5 pr-3 text-muted-foreground max-w-[12rem] truncate">{row.note ?? ''}</td>
                          <td className="py-1.5 pr-3 whitespace-nowrap">{t('taxModels.revenueGroup')}</td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.amountMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.netAmountMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.vatAmountMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.reclaimableVatMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.taxDeductibleCostMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 text-right">
                            <NumberValue value={row.nonTaxDeductibleCostMinor} minorUnits={2} />
                          </td>
                        </tr>
                      ))}
                      <tr className="border-b border-border font-semibold">
                        <td colSpan={3} className="py-1.5 pr-3 text-muted-foreground">
                          {t('taxModels.breakdownTotal')}
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={revenueTotals.amountMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={revenueTotals.netAmountMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={revenueTotals.vatAmountMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={revenueTotals.reclaimableVatMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={revenueTotals.taxDeductibleCostMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 text-right">
                          <NumberValue value={revenueTotals.nonTaxDeductibleCostMinor} minorUnits={2} />
                        </td>
                      </tr>
                    </>
                  )}
                  {expenseRows.length > 0 && (
                    <>
                      <tr className="bg-muted/50">
                        <td colSpan={9} className="py-1 px-1 font-semibold text-foreground">
                          {t('taxModels.expenseGroup')}
                        </td>
                      </tr>
                      {expenseRows.map((row) => (
                        <tr key={row.eventId} className="border-b border-border/40">
                          <td className="py-1.5 pr-3 whitespace-nowrap">{row.eventDate.slice(0, 10)}</td>
                          <td className="py-1.5 pr-3 text-muted-foreground max-w-[12rem] truncate">{row.note ?? ''}</td>
                          <td className="py-1.5 pr-3 whitespace-nowrap">{t('taxModels.expenseGroup')}</td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.amountMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.netAmountMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.vatAmountMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.reclaimableVatMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 pr-3 text-right">
                            <NumberValue value={row.taxDeductibleCostMinor} minorUnits={2} />
                          </td>
                          <td className="py-1.5 text-right">
                            <NumberValue value={row.nonTaxDeductibleCostMinor} minorUnits={2} />
                          </td>
                        </tr>
                      ))}
                      <tr className="border-b border-border font-semibold">
                        <td colSpan={3} className="py-1.5 pr-3 text-muted-foreground">
                          {t('taxModels.breakdownTotal')}
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={expenseTotals.amountMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={expenseTotals.netAmountMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={expenseTotals.vatAmountMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={expenseTotals.reclaimableVatMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 pr-3 text-right">
                          <NumberValue value={expenseTotals.taxDeductibleCostMinor} minorUnits={2} />
                        </td>
                        <td className="py-1.5 text-right">
                          <NumberValue value={expenseTotals.nonTaxDeductibleCostMinor} minorUnits={2} />
                        </td>
                      </tr>
                    </>
                  )}
                </tbody>
              </table>
            </div>
          </details>
        </>
      )}
    </div>
  )
}

export default TaxModelResultsPage
