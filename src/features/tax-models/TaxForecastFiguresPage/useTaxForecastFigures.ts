import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import type { TFunction } from 'i18next'
import { saveTaxForecastFigures } from '@/shared/api'
import { extractErrorMessage } from '@/shared/utils/errors'
import type { ForecastLine, TaxModelDetail } from '@/shared/types'
import { useTaxModelResults } from '../useTaxModelResults'
import {
  draftFromFigures,
  emptyEarlierProfit,
  figuresFromDraft,
  placeholderFor,
  placeholdersCurrent,
  withCell,
  type AmountField,
  type EarlierProfitDraft,
  type FiguresDraftError,
  type Period,
} from './figuresDraft'

const describeField = (t: TFunction, field: AmountField): string => {
  switch (field.target) {
    case 'cell':
      return `${t(`taxModels.forecast.line.${field.line}`)}, ${t(`taxModels.forecast.column.${field.period}`)}`
    case 'payrollMonthly':
      return t('taxModels.figures.payrollMonthly')
    case 'earlierProfit':
      return t('taxModels.figures.earlierProfitRow', { row: field.row })
  }
}

const describeError = (t: TFunction, error: FiguresDraftError): string => {
  const key = `taxModels.figures.validation.${error.kind}`
  if ('field' in error) return t(key, { field: describeField(t, error.field) })
  if (error.kind === 'yearNotBefore') return t(key, { row: error.row, year: error.calendarYear })
  return t(key, { row: error.row })
}

export const useTaxForecastFigures = (model: TaxModelDetail, minorUnits: number) => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const [draft, setDraft] = useState(() => draftFromFigures(model.figures, minorUnits))
  const [saving, setSaving] = useState(false)

  const { data: result } = useTaxModelResults(model.id)
  const saved = draftFromFigures(model.figures, minorUnits)

  const goToResults = () => navigate({ to: '/tax-models/$modelId/results', params: { modelId: String(model.id) } })

  const setMonth = (actualsThroughMonth: number) => setDraft((prev) => ({ ...prev, actualsThroughMonth }))
  const setPayrollMonthly = (payrollMonthly: string) => setDraft((prev) => ({ ...prev, payrollMonthly }))
  const setCell = (line: ForecastLine, period: Period, text: string) => setDraft((prev) => withCell(prev, line, period, text))
  const addEarlierProfit = () => setDraft((prev) => ({ ...prev, earlierProfits: [...prev.earlierProfits, emptyEarlierProfit()] }))
  const changeEarlierProfit = (index: number, patch: Partial<EarlierProfitDraft>) =>
    setDraft((prev) => ({ ...prev, earlierProfits: prev.earlierProfits.map((row, i) => (i === index ? { ...row, ...patch } : row)) }))
  const removeEarlierProfit = (index: number) => setDraft((prev) => ({ ...prev, earlierProfits: prev.earlierProfits.filter((_, i) => i !== index) }))

  const handleSave = async () => {
    const parsed = figuresFromDraft(draft, minorUnits, model.calendarYear)
    if (!parsed.ok) {
      toast.error(describeError(t, parsed.error))
      return
    }

    setSaving(true)
    try {
      await saveTaxForecastFigures({ modelId: model.id, figures: parsed.figures })
      queryClient.invalidateQueries({ queryKey: ['taxModel', model.id] })
      queryClient.invalidateQueries({ queryKey: ['taxModelResults', model.id] })
      goToResults()
    } catch (err) {
      toast.error(extractErrorMessage(err))
    } finally {
      setSaving(false)
    }
  }

  return {
    draft,
    saving,
    placeholder: (line: ForecastLine, period: Period) => (placeholdersCurrent(draft, saved, period) ? placeholderFor(result?.lines, line, period, minorUnits) : undefined),
    setMonth,
    setPayrollMonthly,
    setCell,
    addEarlierProfit,
    changeEarlierProfit,
    removeEarlierProfit,
    handleSave,
    handleCancel: goToResults,
  }
}
