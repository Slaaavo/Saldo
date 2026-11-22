import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChevronRight } from 'lucide-react'
import type { CancelledHoldRow, ImportRow } from '../types'
import { isNewRow } from '../verdicts'
import { cn } from '../../../../shared/lib/utils'
import { Button } from '../../../../shared/ui/button'
import { Checkbox } from '../../../../shared/ui/checkbox'
import { DialogFooter } from '../../../../shared/ui/dialog'
import { ReviewTable, type ReviewTableProps } from './ReviewTable'
import { CancelledHoldsSection } from './CancelledHoldsSection'

export interface ReviewStepProps extends Omit<ReviewTableProps, 'rows' | 'rowsShowingIbanActions'> {
  importRows: ImportRow[]
  unparseableRowCount: number
  declinedRowCount: number
  cancelledHolds: CancelledHoldRow[]
  onToggleCancelledHold: (eventId: number) => void
  selectedCount: number
  canApplyReview: boolean
  balanceWarningDates: string[]
  importing: boolean
  onSelectAll: () => void
  onDeselectAll: () => void
  onImport: () => Promise<void>
  onBack: () => void
  onCancel: () => void
  splitValidationErrors: string[]
}

const rowsShowingIbanActions = (rowsInShownOrder: ImportRow[]): Set<number> => {
  const rows = new Set<number>()
  const seenIbans = new Set<string>()
  for (const row of rowsInShownOrder) {
    if (row.ibanMatch.type === 'unmatched' && row.rawIban && !seenIbans.has(row.rawIban)) {
      seenIbans.add(row.rawIban)
      rows.add(row.index)
    }
  }
  return rows
}

const ReviewStep = ({
  importRows,
  unparseableRowCount,
  declinedRowCount,
  cancelledHolds,
  onToggleCancelledHold,
  selectedCount,
  canApplyReview,
  balanceWarningDates,
  importing,
  onSelectAll,
  onDeselectAll,
  onImport,
  onBack,
  onCancel,
  splitValidationErrors,
  ...tableProps
}: ReviewStepProps) => {
  const { t } = useTranslation()
  const [showKnownRows, setShowKnownRows] = useState(false)

  const newRows = importRows.filter((row) => isNewRow(row.verdict))
  const knownRows = importRows.filter((row) => !isNewRow(row.verdict))

  const newSelectedCount = newRows.filter((row) => row.isSelected).length
  const allSelected = newSelectedCount === newRows.length && newRows.length > 0
  const someSelected = newSelectedCount > 0 && newSelectedCount < newRows.length
  const selectAllState: boolean | 'indeterminate' = allSelected ? true : someSelected ? 'indeterminate' : false

  const handleSelectAll = () => {
    if (allSelected || someSelected) {
      onDeselectAll()
    } else {
      onSelectAll()
    }
  }

  const ibanActionRows = rowsShowingIbanActions([...newRows, ...knownRows])

  return (
    <>
      <div className="flex flex-col gap-3 min-h-0">
        <p className="text-sm text-muted-foreground">{t('import.reviewStep.summary', { newCount: newRows.length, knownCount: knownRows.length })}</p>

        {unparseableRowCount > 0 && (
          <div className="rounded-md border border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-950 p-3 text-sm text-amber-800 dark:text-amber-200">
            {t('import.reviewStep.unparseableRows', { count: unparseableRowCount })}
          </div>
        )}

        {declinedRowCount > 0 && (
          <div className="rounded-md border border-border bg-muted/40 p-3 text-sm text-muted-foreground">{t('import.reviewStep.declinedRows', { count: declinedRowCount })}</div>
        )}

        {balanceWarningDates.length > 0 && (
          <div className="rounded-md border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950 p-3 text-sm text-blue-800 dark:text-blue-200">
            {t('import.reviewStep.balanceWarning', {
              dates: balanceWarningDates.join(', '),
            })}
          </div>
        )}

        {newRows.length > 0 && (
          <>
            <div className="flex items-center gap-2">
              <Checkbox checked={selectAllState} onCheckedChange={handleSelectAll} id="select-all" />
              <label htmlFor="select-all" className="text-sm cursor-pointer select-none">
                {newSelectedCount > 0 ? t('import.reviewStep.selectAll') + ` (${newSelectedCount}/${newRows.length})` : t('import.reviewStep.selectAll')}
              </label>
            </div>
            <ReviewTable rows={newRows} rowsShowingIbanActions={ibanActionRows} {...tableProps} />
          </>
        )}

        {knownRows.length > 0 && (
          <div className="flex flex-col gap-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="self-start px-1 text-muted-foreground"
              aria-expanded={showKnownRows}
              onClick={() => setShowKnownRows((shown) => !shown)}
            >
              <ChevronRight className={cn('h-4 w-4 transition-transform', showKnownRows && 'rotate-90')} />
              {t('import.reviewStep.alreadyInSaldo', { count: knownRows.length })}
            </Button>
            {showKnownRows && <ReviewTable rows={knownRows} rowsShowingIbanActions={ibanActionRows} {...tableProps} />}
          </div>
        )}

        {cancelledHolds.length > 0 && (
          <CancelledHoldsSection
            holds={cancelledHolds}
            currencyCode={tableProps.selectedAccountCurrencyCode}
            minorUnits={tableProps.selectedAccountMinorUnits}
            onToggle={onToggleCancelledHold}
          />
        )}
      </div>

      {splitValidationErrors.length > 0 && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          <p className="font-medium">{t('import.reviewStep.split.validationBanner')}</p>
          {splitValidationErrors.map((err, i) => (
            <p key={i} className="text-xs mt-1">
              {err}
            </p>
          ))}
        </div>
      )}

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack} disabled={importing}>
          {t('import.back')}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel} disabled={importing}>
          {t('modals.confirm.cancel')}
        </Button>
        <Button type="button" onClick={onImport} disabled={!canApplyReview}>
          {importing ? t('import.reviewStep.importing') : selectedCount > 0 ? t('import.reviewStep.importButton', { count: selectedCount }) : t('import.reviewStep.finish')}
        </Button>
      </DialogFooter>
    </>
  )
}

export default ReviewStep
