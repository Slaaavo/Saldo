import { useRef } from 'react'
import { useTranslation } from 'react-i18next'
import type { AsOf, IncomeBudget, SnapshotRow } from '@/shared/types'
import { Button } from '@/shared/ui/button'
import { DialogBody, DialogFooter } from '@/shared/ui/dialog'
import { useConsolidationCurrencyQuery } from '@/shared/hooks/useConsolidationCurrencyQuery'
import { formatShortDay } from '@/shared/utils/format'
import { useIncomeBudgetEditor } from '../useIncomeBudgetEditor'
import { useIncomeBudgetPreview } from '../useIncomeBudgetQueries'
import { BudgetSummary } from './BudgetSummary'
import { BudgetTable } from './BudgetTable'
import { CountedAccountsLine } from './CountedAccountsLine'
import { surfaceOf } from './tableSurface'

interface Props {
  row: SnapshotRow
  budget: IncomeBudget
  asOf: AsOf
  personName: string
  onClose: () => void
}

export const BudgetEditor = ({ row, budget, asOf, personName, onClose }: Props) => {
  const { t, i18n } = useTranslation()
  const scrollerRef = useRef<HTMLDivElement>(null)
  const bucketCurrency = { code: row.currencyCode, minorUnits: row.currencyMinorUnits }
  const editor = useIncomeBudgetEditor({
    bucketId: row.accountId,
    budget,
    minorUnits: bucketCurrency.minorUnits,
    measure: () => (scrollerRef.current ? surfaceOf(scrollerRef.current) : null),
    onSaved: onClose,
  })
  const { data: preview } = useIncomeBudgetPreview({ bucketId: row.accountId, asOf, span: editor.span, draft: editor.draft, minorUnits: bucketCurrency.minorUnits })
  const { data: consolidation } = useConsolidationCurrencyQuery()
  const lines = new Map(preview?.lines.map((line) => [line.month, line]))
  const momentLabel = asOf.kind === 'live' ? t('incomeBudget.table.today') : t('incomeBudget.table.asOf', { date: formatShortDay(budget.day, i18n.language) })

  return (
    <>
      <DialogBody className="flex flex-col gap-4">
        <BudgetSummary
          preview={preview}
          bucketCurrency={bucketCurrency}
          consolidationCurrency={consolidation ?? bucketCurrency}
          enabled={row.bucket?.enabled ?? true}
          moment={budget.momentMonth}
          day={budget.day}
        />
        <BudgetTable
          scrollerRef={scrollerRef}
          draft={editor.draft}
          span={editor.span}
          months={editor.months}
          lines={lines}
          moment={budget.momentMonth}
          momentLabel={momentLabel}
          currencyCode={bucketCurrency.code}
          minorUnits={bucketCurrency.minorUnits}
          drag={editor.drag}
          canAddYear={editor.canAddYear}
          onCellChange={editor.changeCell}
          onFillStart={editor.startFill}
          onAddYear={editor.addYear}
        />
        <CountedAccountsLine accounts={budget.countedAccounts} personName={personName} currencyCode={bucketCurrency.code} />
        <p aria-live="polite" className="min-h-5 text-xs font-semibold text-accent-foreground">
          {editor.announcement}
        </p>
      </DialogBody>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          {t('incomeBudget.cancel')}
        </Button>
        <Button type="button" onClick={() => void editor.save()} disabled={editor.saving}>
          {t('incomeBudget.save')}
        </Button>
      </DialogFooter>
    </>
  )
}
