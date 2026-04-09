import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from '@tanstack/react-router'
import { Button } from '../../shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '../../shared/ui/card'
import NumberValue from '../../shared/ui/NumberValue'
import { getTaxModel } from '../../shared/api'
import { usePageTitle } from '../../app/usePageHeader'

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

  usePageTitle(model != null ? `${model.name} – ${model.calendarYear}` : t('taxModels.resultsTitle'))

  if (isLoading) return null

  const isLegal = model?.personType === 'legal'

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

      <p className="text-sm text-muted-foreground mb-6">{t('taxModels.mockDataNotice')}</p>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t('taxModels.resultsTitle')}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-sm">
            <div className="flex justify-between py-2 border-b border-border">
              <span>{t('taxModels.totalIncome')}</span>
              <NumberValue value={12000000} minorUnits={2} />
            </div>
            <div className="flex justify-between py-2 border-b border-border">
              <span className="font-medium">{t('taxModels.totalExpenses')}</span>
            </div>
            <div className="flex justify-between py-2 border-b border-border pl-4">
              <span className="text-muted-foreground">{t('taxModels.deductibleExpenses')}</span>
              <NumberValue value={4800000} minorUnits={2} />
            </div>
            <div className="flex justify-between py-2 border-b border-border pl-4">
              <span className="text-muted-foreground">{t('taxModels.nonDeductibleExpenses')}</span>
              <NumberValue value={1200000} minorUnits={2} />
            </div>
            <div className="flex justify-between py-2 border-b border-border">
              <span>{t('taxModels.grossProfit')}</span>
              <NumberValue value={6000000} minorUnits={2} />
            </div>
            <div className="flex justify-between py-2 border-b border-border">
              <span>{t('taxModels.totalTax')}</span>
              <NumberValue value={1710000} minorUnits={2} />
            </div>
            <div className="flex justify-between py-2 border-b border-border">
              <span>{t('taxModels.netProfit')}</span>
              <NumberValue value={4290000} minorUnits={2} />
            </div>
            {isLegal && (
              <>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.reserveFundAllocation')}</span>
                  <NumberValue value={210000} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.dividendAmount')}</span>
                  <NumberValue value={4080000} minorUnits={2} />
                </div>
                <div className="flex justify-between py-2 border-b border-border">
                  <span>{t('taxModels.withholdingTax')}</span>
                  <NumberValue value={183600} minorUnits={2} />
                </div>
              </>
            )}
            <div className="flex justify-between py-2 border-b border-border">
              <span>{t('taxModels.monthlyTaxBurden')}</span>
              <NumberValue value={142500} minorUnits={2} />
            </div>
            <div className="flex justify-between py-2 last:border-0">
              <span>{t('taxModels.netPayout')}</span>
              <NumberValue value={358825} minorUnits={2} />
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

export default TaxModelResultsPage
