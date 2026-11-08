import { useDeferredValue } from 'react'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { getIncomeBudget, previewIncomeBudget } from '@/shared/api'
import type { AsOf } from '@/shared/types'
import { parseDraft, type BudgetDraft, type YearSpan } from './budgetDraft'

export const useIncomeBudget = (bucketId: number, asOf: AsOf) =>
  useQuery({
    queryKey: ['snapshot', 'income-budget', bucketId, asOf],
    queryFn: () => getIncomeBudget(bucketId, asOf),
    // The editor seeds its draft once from this data, so a reopened dialog must never start from an older cache.
    gcTime: 0,
  })

interface PreviewOptions {
  bucketId: number
  asOf: AsOf
  span: YearSpan
  draft: BudgetDraft
  minorUnits: number
}

export const useIncomeBudgetPreview = ({ bucketId, asOf, span, draft, minorUnits }: PreviewOptions) => {
  const deferredDraft = useDeferredValue(draft)
  const { fromYear, toYear } = useDeferredValue(span)
  const { cells } = parseDraft(deferredDraft, minorUnits)
  return useQuery({
    queryKey: ['snapshot', 'income-budget-preview', bucketId, asOf, fromYear, toYear, cells],
    queryFn: () => previewIncomeBudget({ bucketId, asOf, fromYear, toYear, months: cells }),
    placeholderData: keepPreviousData,
    gcTime: 0,
  })
}
