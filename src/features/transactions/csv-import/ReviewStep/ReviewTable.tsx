import { Fragment, type ReactElement, type ReactNode } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import { AlertTriangle } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ImportRow, IbanMatchResult } from '../types'
import type { SnapshotRow } from '../../../../shared/types'
import { cn } from '../../../../shared/lib/utils'
import { Button } from '../../../../shared/ui/button'
import { Checkbox } from '../../../../shared/ui/checkbox'
import NumberValue from '../../../../shared/ui/NumberValue'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'
import { PartnerCell } from './PartnerCell'
import { SplitEditorPanel } from './SplitEditorPanel'

export interface ReviewTableProps {
  rows: ImportRow[]
  rowsShowingIbanActions: Set<number>
  availableBuckets: SnapshotRow[]
  accountsWithoutIban: SnapshotRow[]
  counterpartAccounts: SnapshotRow[]
  selectedAccountCurrencyCode: string
  selectedAccountMinorUnits: number
  onToggleRow: (index: number) => void
  onBucketChange: (index: number, bucketId: number | null) => void
  onCreatePartner: (iban: string, name: string) => Promise<boolean>
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>
  onCounterpartChange: (index: number, accountId: number | null) => void
  onSplitOpen: (rowIndex: number) => void
  onSplitCancel: (rowIndex: number) => void
  onLegAmountChange: (rowIndex: number, legId: number, amountMinor: number) => void
  onLegNoteChange: (rowIndex: number, legId: number, note: string | null) => void
  onLegPartnerChange: (rowIndex: number, legId: number, rawIban: string | null, ibanMatch: IbanMatchResult, counterpartAccountId: number | null) => void
  onLegBucketChange: (rowIndex: number, legId: number, bucketId: number | null) => void
  onAddLeg: (rowIndex: number) => void
  onRemoveLeg: (rowIndex: number, legId: number) => void
}

const BUCKET_NONE = '__none__'

const dayOf = (eventDate: string) => eventDate.substring(0, 10)

const hintOf = (t: TFunction, row: ImportRow, money: (valueMinor: number) => ReactElement): ReactNode => {
  const { verdict } = row
  switch (verdict.kind) {
    case 'holdPosted':
      return row.isSelected
        ? t('import.reviewStep.holdPosted.staysPending', { day: dayOf(verdict.eventDate) })
        : t('import.reviewStep.holdPosted.posts', { day: dayOf(verdict.eventDate) })
    case 'holdPostedAtOtherAmount':
      return row.isSelected ? (
        <Trans i18nKey="import.reviewStep.holdPostedAtOtherAmount.posts" components={{ hold: money(verdict.holdAmountMinor), posted: money(row.amountMinor) }} />
      ) : (
        <Trans i18nKey="import.reviewStep.holdPostedAtOtherAmount.stays" components={{ hold: money(verdict.holdAmountMinor) }} />
      )
    case 'newReplacingSplitHold':
      return (
        <span className="flex items-start gap-1 text-amber-800 dark:text-amber-200">
          <AlertTriangle className="h-3.5 w-3.5 mt-px text-amber-500 shrink-0" />
          <span>
            <Trans
              i18nKey="import.reviewStep.newReplacingSplitHold"
              values={{ day: dayOf(verdict.eventDate) }}
              components={{ hold: money(verdict.holdAmountMinor), booked: money(row.amountMinor) }}
            />
          </span>
        </span>
      )
    case 'completesTransfer':
      if (row.splitLegs !== null) return null
      return t('import.reviewStep.completesTransfer', { account: verdict.accountName, day: dayOf(verdict.eventDate) })
    case 'transferLeg':
      return row.isSelected
        ? t('import.reviewStep.transferLeg.stays', { account: verdict.otherAccountName, from: dayOf(verdict.eventDate) })
        : t('import.reviewStep.transferLeg.moves', { account: verdict.otherAccountName, from: dayOf(verdict.eventDate), to: dayOf(row.date) })
    case 'new':
    case 'inSaldo':
      return null
  }
}

const ReviewTableColGroup = () => {
  return (
    <colgroup>
      <col className="w-8" />
      <col className="w-24" />
      <col className="w-40" />
      <col />
      <col className="w-32" />
      <col className="w-40" />
      <col className="w-20" />
    </colgroup>
  )
}

export const ReviewTable = ({
  rows,
  rowsShowingIbanActions,
  availableBuckets,
  accountsWithoutIban,
  counterpartAccounts,
  selectedAccountCurrencyCode,
  selectedAccountMinorUnits,
  onToggleRow,
  onBucketChange,
  onCreatePartner,
  onAssignIban,
  onCounterpartChange,
  onSplitOpen,
  onSplitCancel,
  onLegAmountChange,
  onLegNoteChange,
  onLegPartnerChange,
  onLegBucketChange,
  onAddLeg,
  onRemoveLeg,
}: ReviewTableProps) => {
  const { t } = useTranslation()
  const money = (valueMinor: number) => <NumberValue value={valueMinor} minorUnits={selectedAccountMinorUnits} currencyCode={selectedAccountCurrencyCode} />

  return (
    <div className="border border-border rounded-[var(--radius)] bg-background overflow-hidden">
      <table className="w-full text-sm table-fixed border-separate border-spacing-0">
        <ReviewTableColGroup />
        <thead>
          <tr>
            <th className="px-2 py-2 border-b border-border" />
            <th className="px-2 py-2 text-left font-medium text-muted-foreground whitespace-nowrap border-b border-border">{t('import.mappingStep.field.date')}</th>
            <th className="px-2 py-2 text-right font-medium text-muted-foreground whitespace-nowrap border-b border-border">{t('import.mappingStep.field.amount')}</th>
            <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">{t('import.mappingStep.field.partner')}</th>
            <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">{t('import.mappingStep.field.note')}</th>
            <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">{t('import.reviewStep.bucket')}</th>
            <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">{t('import.reviewStep.split.actionsColumn')}</th>
          </tr>
        </thead>
      </table>
      <div className="max-h-[360px] overflow-y-auto [scrollbar-gutter:stable]">
        <table className="w-full text-sm table-fixed border-separate border-spacing-0">
          <ReviewTableColGroup />
          <tbody>
            {rows.map((row) => {
              const isSplit = row.splitLegs !== null
              const bucketAndSplitLocked = row.verdict.kind === 'holdPostedAtOtherAmount'
              const hint = hintOf(t, row, money)
              const cell = cn('px-2 py-2', hint === null && 'border-b border-border group-last:border-b-0')
              return (
                <Fragment key={row.index}>
                  <tr className={cn('group align-top', !row.isSelected && 'opacity-50', isSplit && 'bg-muted/30')}>
                    <td className={cell}>
                      <Checkbox checked={row.isSelected} onCheckedChange={() => onToggleRow(row.index)} />
                    </td>
                    <td className={cn(cell, 'whitespace-nowrap')}>{row.date.substring(0, 10)}</td>
                    <td className={cn(cell, 'text-right whitespace-nowrap')}>
                      <NumberValue value={row.amountMinor} minorUnits={selectedAccountMinorUnits} currencyCode={selectedAccountCurrencyCode} />
                    </td>
                    <td className={cell}>
                      {isSplit ? (
                        <span className="text-muted-foreground">—</span>
                      ) : (
                        <PartnerCell
                          row={row}
                          accountsWithoutIban={accountsWithoutIban}
                          counterpartAccounts={counterpartAccounts}
                          onCreatePartner={onCreatePartner}
                          onAssignIban={onAssignIban}
                          onCounterpartChange={onCounterpartChange}
                          isFirstOccurrence={rowsShowingIbanActions.has(row.index)}
                        />
                      )}
                    </td>
                    <td className={cell}>
                      {row.pending && (
                        <span className="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium bg-amber-100 text-amber-800 dark:bg-amber-900 dark:text-amber-200">
                          {t('import.reviewStep.cardHold')}
                        </span>
                      )}
                      <span className="text-muted-foreground truncate block" title={row.note ?? ''}>
                        {row.note ?? '—'}
                      </span>
                    </td>
                    <td className={cell}>
                      {isSplit || bucketAndSplitLocked ? (
                        <span className="text-muted-foreground">—</span>
                      ) : (
                        <Select
                          value={row.bucketId !== null ? String(row.bucketId) : BUCKET_NONE}
                          onValueChange={(v) => onBucketChange(row.index, v === BUCKET_NONE ? null : Number(v))}
                        >
                          <SelectTrigger className="h-7 text-xs">
                            <SelectValue placeholder={t('import.reviewStep.selectBucket')} />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value={BUCKET_NONE}>{t('import.reviewStep.selectBucket')}</SelectItem>
                            {availableBuckets.map((b) => (
                              <SelectItem key={b.accountId} value={String(b.accountId)}>
                                {b.accountName}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      )}
                    </td>
                    <td className={cell}>
                      {isSplit ? (
                        <Button type="button" variant="ghost" size="sm" className="h-7 text-xs px-2" onClick={() => onSplitCancel(row.index)}>
                          {t('import.reviewStep.split.cancelButton')}
                        </Button>
                      ) : bucketAndSplitLocked ? (
                        <span className="text-muted-foreground">—</span>
                      ) : (
                        <Button type="button" variant="ghost" size="sm" className="h-7 text-xs px-2" onClick={() => onSplitOpen(row.index)}>
                          {t('import.reviewStep.split.splitButton')}
                        </Button>
                      )}
                    </td>
                  </tr>
                  {hint !== null && (
                    <tr className={cn('group', !row.isSelected && 'opacity-50', isSplit && 'bg-muted/30')}>
                      <td className="border-b border-border group-last:border-b-0" />
                      <td colSpan={6} className="px-2 pb-2 text-xs text-muted-foreground border-b border-border group-last:border-b-0">
                        {hint}
                      </td>
                    </tr>
                  )}
                  {isSplit && (
                    <tr>
                      <td colSpan={7} className="p-0">
                        <SplitEditorPanel
                          row={row}
                          selectedAccountCurrencyCode={selectedAccountCurrencyCode}
                          selectedAccountMinorUnits={selectedAccountMinorUnits}
                          availableBuckets={availableBuckets}
                          accountsWithoutIban={accountsWithoutIban}
                          counterpartAccounts={counterpartAccounts}
                          onLegAmountChange={onLegAmountChange}
                          onLegNoteChange={onLegNoteChange}
                          onLegPartnerChange={onLegPartnerChange}
                          onLegBucketChange={onLegBucketChange}
                          onAddLeg={onAddLeg}
                          onRemoveLeg={onRemoveLeg}
                          onCreatePartner={onCreatePartner}
                          onAssignIban={onAssignIban}
                        />
                      </td>
                    </tr>
                  )}
                </Fragment>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
