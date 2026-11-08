import { useLayoutEffect, useRef, type PointerEvent, type ReactNode, type RefObject } from 'react'
import { useTranslation } from 'react-i18next'
import type { MonthKey, MonthLine } from '@/shared/types'
import { cn } from '@/shared/lib/utils'
import { defaultNumberFormat, type NumberFormatConfig } from '@/shared/config/numberFormat'
import NumberValue from '@/shared/ui/NumberValue'
import { DRAFT_ROWS, cellText, parseCell, type BudgetDraft, type DraftRow, type YearSpan } from '../budgetDraft'
import { scrollLeftForMoment, type FillSpan } from '../fillRange'
import { cellLabel, fillLabel } from '../labels'
import { formatMonth } from '../month'
import { BudgetInputCell } from './BudgetInputCell'
import { YearEdgeButton } from './YearEdgeButton'
import { layoutOf } from './tableSurface'

const BARE_NUMBER: NumberFormatConfig = { ...defaultNumberFormat, currencySymbol: '' }

interface Props {
  scrollerRef: RefObject<HTMLDivElement | null>
  draft: BudgetDraft
  span: YearSpan
  months: MonthKey[]
  lines: ReadonlyMap<MonthKey, MonthLine>
  moment: MonthKey
  momentLabel: string
  currencyCode: string
  minorUnits: number
  drag: FillSpan | null
  canAddYear: { before: boolean; after: boolean }
  onCellChange: (month: MonthKey, row: DraftRow, text: string) => void
  onFillStart: (row: DraftRow, index: number, event: PointerEvent<HTMLButtonElement>) => void
  onAddYear: (side: 'before' | 'after') => void
}

type ScrollIntent = 'moment' | 'start' | 'end'

const LabelCell = ({ label, unit }: { label: string; unit: string }) => (
  <th scope="row" className="sticky left-0 z-20 w-[148px] min-w-[148px] border-r border-b border-border bg-card px-3 py-2 text-left font-semibold whitespace-nowrap">
    {label}
    <small className="block text-[11px] font-medium text-muted-foreground">{unit}</small>
  </th>
)

const ComputedCell = ({ className, children }: { className?: string; children?: ReactNode }) => (
  <td className={cn('border-b border-border px-2.5 py-2.5 text-right whitespace-nowrap', className)}>{children}</td>
)

export const BudgetTable = ({
  scrollerRef,
  draft,
  span,
  months,
  lines,
  moment,
  momentLabel,
  currencyCode,
  minorUnits,
  drag,
  canAddYear,
  onCellChange,
  onFillStart,
  onAddYear,
}: Props) => {
  const { t, i18n } = useTranslation()
  const language = i18n.language
  const pendingScroll = useRef<ScrollIntent | null>('moment')

  useLayoutEffect(() => {
    const scroller = scrollerRef.current
    const intent = pendingScroll.current
    if (!scroller || !intent) return
    pendingScroll.current = null
    if (intent === 'moment') {
      scroller.scrollLeft = scrollLeftForMoment(layoutOf(scroller), months, moment)
      return
    }
    // WKWebView drops a scroll position set in the same frame as the year's new columns, so it keeps painting the old offset.
    requestAnimationFrame(() => {
      scroller.scrollLeft = intent === 'start' ? 0 : scroller.scrollWidth
    })
  })

  const addYear = (side: 'before' | 'after') => {
    pendingScroll.current = side === 'before' ? 'start' : 'end'
    onAddYear(side)
  }

  const columnClass = (month: MonthKey) => cn(month.endsWith('-01') && 'border-l border-border', month === moment && 'bg-accent')
  const inFill = (row: DraftRow, index: number) =>
    drag !== null && drag.row === row && index >= Math.min(drag.originIdx, drag.targetIdx) && index <= Math.max(drag.originIdx, drag.targetIdx)
  const soFar = (month: MonthKey) => month === moment && <span className="block text-[10px] font-medium text-muted-foreground">{t('incomeBudget.table.soFar')}</span>

  const inputRow = (row: DraftRow) =>
    months.map((month, index) => {
      const text = cellText(draft, month, row)
      const budgeted = row === 'vat' && cellText(draft, month, 'budget').trim() !== ''
      return (
        <BudgetInputCell
          key={month}
          text={text}
          label={cellLabel(t, language, row, month)}
          fillLabel={fillLabel(t, language, row, month)}
          invalid={parseCell(row, text, minorUnits).kind === 'invalid'}
          placeholder={budgeted ? '0' : undefined}
          className={columnClass(month)}
          inFill={inFill(row, index)}
          isFillOrigin={drag !== null && drag.row === row && drag.originIdx === index}
          onChange={(next) => onCellChange(month, row, next)}
          onFillStart={(event) => onFillStart(row, index, event)}
        />
      )
    })

  const rowUnits: Record<DraftRow, string> = {
    budget: t('incomeBudget.table.withoutVat', { currency: currencyCode }),
    vat: '%',
  }

  return (
    <div className="grid grid-cols-[auto_minmax(0,1fr)_auto] items-stretch gap-2">
      <YearEdgeButton year={span.fromYear - 1} disabled={!canAddYear.before} onClick={() => addYear('before')} />
      <div ref={scrollerRef} className={cn('min-w-0 overflow-x-auto rounded-md border border-border bg-card', drag && 'select-none')}>
        <table className="border-separate border-spacing-0 text-sm tabular-nums">
          <thead>
            <tr>
              <th scope="col" data-label-column="" className="sticky left-0 z-20 w-[148px] min-w-[148px] border-r border-b border-border bg-card px-3 py-2 text-left font-semibold">
                {t('incomeBudget.table.month')}
              </th>
              {months.map((month) => (
                <th
                  key={month}
                  scope="col"
                  data-month={month}
                  className={cn(
                    'min-w-[92px] border-b border-border px-2.5 py-2 text-right align-top text-xs font-semibold whitespace-nowrap text-muted-foreground select-none',
                    columnClass(month),
                  )}
                >
                  {formatMonth(month, language, 'short')}
                  {month === moment && <span className="block text-[10px] font-bold tracking-wide text-primary uppercase">{momentLabel}</span>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {DRAFT_ROWS.map((row) => (
              <tr key={row}>
                <LabelCell label={t(`incomeBudget.table.${row}`)} unit={rowUnits[row]} />
                {inputRow(row)}
              </tr>
            ))}
            <tr className="[&>*]:border-t-2 [&>*]:border-t-border">
              <LabelCell label={t('incomeBudget.table.received')} unit={t('incomeBudget.table.withoutVat', { currency: currencyCode })} />
              {months.map((month) => {
                const line = lines.get(month)
                const received = line?.receivedMinor ?? null
                return (
                  <ComputedCell key={month} className={cn(columnClass(month), line && !line.budgeted && 'text-muted-foreground')}>
                    {received !== null && (
                      <>
                        <NumberValue value={received} minorUnits={minorUnits} config={BARE_NUMBER} />
                        {soFar(month)}
                      </>
                    )}
                  </ComputedCell>
                )
              })}
            </tr>
            <tr className="[&>*]:border-b-0">
              <LabelCell label={t('incomeBudget.table.difference')} unit={currencyCode} />
              {months.map((month) => {
                const difference = lines.get(month)?.differenceMinor ?? null
                const tone = difference === null || difference === 0 ? 'text-muted-foreground' : difference > 0 ? 'text-success' : 'text-destructive'
                return (
                  <ComputedCell key={month} className={cn(columnClass(month), 'font-semibold', tone)}>
                    {difference !== null && (
                      <>
                        {difference > 0 && '+'}
                        <NumberValue value={difference} minorUnits={minorUnits} config={BARE_NUMBER} />
                        {soFar(month)}
                      </>
                    )}
                  </ComputedCell>
                )
              })}
            </tr>
          </tbody>
        </table>
      </div>
      <YearEdgeButton year={span.toYear + 1} disabled={!canAddYear.after} onClick={() => addYear('after')} />
    </div>
  )
}
