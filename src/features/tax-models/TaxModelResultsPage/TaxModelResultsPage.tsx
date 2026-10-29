import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate, useParams } from '@tanstack/react-router'
import { Button } from '@/shared/ui/button'
import { getTaxModel } from '@/shared/api'
import { usePageTitle } from '@/app/usePageHeader'
import { useConsolidationCurrencyQuery } from '@/shared/hooks/useConsolidationCurrencyQuery'
import { useTaxModelResults } from '../useTaxModelResults'
import { ForecastSheet } from './ForecastSheet'
import { ResultSummary } from './ResultSummary'
import { EntryBreakdown } from './EntryBreakdown'

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

  const { data: result, isFetching: isResultFetching } = useTaxModelResults(Number(modelId), !!modelId)

  const { data: consolidationCurrency, isLoading: isCurrencyLoading } = useConsolidationCurrencyQuery()
  const minorUnits = consolidationCurrency?.minorUnits ?? 2
  const currencyCode = consolidationCurrency?.code

  usePageTitle(model != null ? `${model.name} – ${model.calendarYear}` : t('taxModels.resultsTitle'))

  if (isLoading || isResultFetching || isCurrencyLoading) return null

  return (
    <div className="px-4 md:px-10 py-8 max-w-4xl mx-auto">
      <div className="flex items-center gap-4 mb-4">
        <Button variant="ghost" size="sm" onClick={() => navigate({ to: '/tax-models' })}>
          ← {t('taxModels.backToList')}
        </Button>
        <div className="ml-auto flex gap-2">
          <Button variant="outline" size="sm" onClick={() => navigate({ to: '/tax-models/$modelId/figures', params: { modelId: modelId! } })}>
            {t('taxModels.updateFigures')}
          </Button>
          <Button variant="outline" size="sm" onClick={() => navigate({ to: '/tax-models/$modelId/edit', params: { modelId: modelId! } })}>
            {t('taxModels.adjustSettings')}
          </Button>
        </div>
      </div>

      {result != null && model != null && (
        <div className="space-y-6">
          <ForecastSheet result={result} minorUnits={minorUnits} currencyCode={currencyCode} />
          <ResultSummary result={result} calendarYear={model.calendarYear} minorUnits={minorUnits} currencyCode={currencyCode} />
          {result.eventBreakdowns.length > 0 && <EntryBreakdown rows={result.eventBreakdowns} minorUnits={minorUnits} currencyCode={currencyCode} />}
        </div>
      )}
    </div>
  )
}

export default TaxModelResultsPage
