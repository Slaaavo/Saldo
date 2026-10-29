import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useParams } from '@tanstack/react-router'
import { getTaxModel } from '@/shared/api'
import { usePageTitle } from '@/app/usePageHeader'
import { useConsolidationCurrencyQuery } from '@/shared/hooks/useConsolidationCurrencyQuery'
import { FiguresForm } from './FiguresForm'

const TaxForecastFiguresPage = () => {
  const { t } = useTranslation()
  const params = useParams({ strict: false }) as { modelId?: string }
  const modelId = Number(params.modelId)

  const { data: model, isFetchedAfterMount } = useQuery({
    queryKey: ['taxModel', modelId],
    queryFn: () => getTaxModel(modelId),
    enabled: params.modelId !== undefined,
    refetchOnMount: 'always',
  })
  const { data: currency } = useConsolidationCurrencyQuery()

  usePageTitle(t('taxModels.figures.title'))

  if (!isFetchedAfterMount || model == null || currency == null) return null

  return <FiguresForm key={model.id} model={model} minorUnits={currency.minorUnits} currencyCode={currency.code} />
}

export default TaxForecastFiguresPage
