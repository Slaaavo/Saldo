import { useDeferredValue } from 'react'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { getIncomeBudget, previewIncomeBudget } from '@/shared/api'
import type { AsOf } from '@/shared/types'
import { parseDraft, type BudgetDraft, type YearSpan } from './budgetDraft'

export const useIncomeBudget = (bucketId: number, asOf: AsOf) =>
  useQuery({
    queryKey: ['incomeBudget', bucketId, asOf],
    queryFn: () => getIncomeBudget(bucketId, asOf),
  })

interface PreviewOptions {
  bucketId: number
  asOf: AsOf
  span: YearSpan
  draft: BudgetDraft
  minorUnits: number
}

/** Received depends on cashflows, so the key nests under 'events' and refreshes with every entry change. */
export const useIncomeBudgetPreview = ({ bucketId, asOf, span, draft, minorUnits }: PreviewOptions) => {
  const deferredDraft = useDeferredValue(draft)
  const { fromYear, toYear } = useDeferredValue(span)
  const { cells } = parseDraft(deferredDraft, minorUnits)
  return useQuery({
    queryKey: ['events', 'income-budget-preview', bucketId, asOf, fromYear, toYear, cells],
    queryFn: () => previewIncomeBudget({ bucketId, asOf, fromYear, toYear, months: cells }),
    placeholderData: keepPreviousData,
  })
}
