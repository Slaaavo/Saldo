import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { saveIncomeBudget } from '@/shared/api'
import type { IncomeBudget, MonthKey } from '@/shared/types'
import { extractErrorMessage } from '@/shared/utils/errors'
import { cellText, draftFromCells, parseDraft, setCell, spanOf, widen, type DraftRow } from './budgetDraft'
import { fillRange, type FillSpan } from './fillRange'
import { cellLabel } from './labels'
import { monthsOfYears, yearOf } from './month'
import { useFillDrag, type DragSurface } from './useFillDrag'

interface Options {
  bucketId: number
  budget: IncomeBudget
  minorUnits: number
  measure: () => DragSurface | null
  onSaved: () => void
}

/** The dialog's draft from open to Save. Nothing reaches the database before Save, so Cancel only has to drop it. */
export const useIncomeBudgetEditor = ({ bucketId, budget, minorUnits, measure, onSaved }: Options) => {
  const { t, i18n } = useTranslation()
  const queryClient = useQueryClient()
  const [draft, setDraft] = useState(() => draftFromCells(budget.months, minorUnits))
  const [span, setSpan] = useState(() => spanOf(draft, yearOf(budget.momentMonth)))
  const [saving, setSaving] = useState(false)
  const [announcement, setAnnouncement] = useState('')
  const months = monthsOfYears(span.fromYear, span.toYear)

  const changeCell = (month: MonthKey, row: DraftRow, text: string) => setDraft((prev) => setCell(prev, month, row, text))

  const fill = (fillSpan: FillSpan) => {
    const text = cellText(draft, months[fillSpan.originIdx], fillSpan.row).trim()
    const count = Math.abs(fillSpan.targetIdx - fillSpan.originIdx)
    setDraft((prev) => fillRange(prev, months, fillSpan))
    setAnnouncement(text === '' ? t('incomeBudget.announce.cleared', { count }) : t('incomeBudget.announce.filled', { count, value: text }))
  }

  const { drag, start: startFill } = useFillDrag({ measure, onFill: fill })

  const addYear = (side: 'before' | 'after') => {
    const wider = widen(span, side)
    if (!wider) return
    setSpan(wider)
    setAnnouncement(t('incomeBudget.announce.yearAdded', { year: side === 'before' ? wider.fromYear : wider.toYear }))
  }

  const save = async () => {
    const { cells, invalid } = parseDraft(draft, minorUnits)
    const [problem] = invalid
    if (problem) {
      toast.error(t(`incomeBudget.invalid.${problem.row}`, { cell: cellLabel(t, i18n.language, problem.row, problem.month) }))
      return
    }
    setSaving(true)
    try {
      await saveIncomeBudget(bucketId, cells)
      onSaved()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['incomeBudget', bucketId] })
    } catch (err) {
      toast.error(t('errors.saveIncomeBudget', { error: extractErrorMessage(err) }))
    } finally {
      setSaving(false)
    }
  }

  return {
    draft,
    span,
    months,
    canAddYear: { before: widen(span, 'before') !== null, after: widen(span, 'after') !== null },
    changeCell,
    addYear,
    drag,
    startFill,
    save,
    saving,
    announcement,
  }
}
