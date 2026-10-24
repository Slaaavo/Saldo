import { useTranslation } from 'react-i18next'
import { Check } from 'lucide-react'
import type { ImportRow, IbanMatchResult } from '../types'
import type { SnapshotRow } from '../../../../shared/types'
import { cn } from '../../../../shared/lib/utils'
import { Button } from '../../../../shared/ui/button'
import NumberValue from '../../../../shared/ui/NumberValue'
import { SplitLegRow } from './SplitLegRow'

const SplitEditorColGroup = () => {
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

interface SplitEditorPanelProps {
  row: ImportRow
  selectedAccountCurrencyCode: string
  selectedAccountMinorUnits: number
  availableBuckets: SnapshotRow[]
  accountsWithoutIban: SnapshotRow[]
  counterpartAccounts: SnapshotRow[]
  onLegAmountChange: (rowIndex: number, legId: number, amountMinor: number) => void
  onLegNoteChange: (rowIndex: number, legId: number, note: string | null) => void
  onLegPartnerChange: (rowIndex: number, legId: number, rawIban: string | null, ibanMatch: IbanMatchResult, counterpartAccountId: number | null) => void
  onLegBucketChange: (rowIndex: number, legId: number, bucketId: number | null) => void
  onAddLeg: (rowIndex: number) => void
  onRemoveLeg: (rowIndex: number, legId: number) => void
  onCreatePartner: (iban: string, name: string) => Promise<boolean>
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>
}

export const SplitEditorPanel = ({
  row,
  selectedAccountCurrencyCode,
  selectedAccountMinorUnits,
  availableBuckets,
  accountsWithoutIban,
  counterpartAccounts,
  onLegAmountChange,
  onLegNoteChange,
  onLegPartnerChange,
  onLegBucketChange,
  onAddLeg,
  onRemoveLeg,
  onCreatePartner,
  onAssignIban,
}: SplitEditorPanelProps) => {
  const { t } = useTranslation()

  const legs = row.splitLegs!
  const allocatedMinor = legs.reduce((sum, l) => sum + l.amountMinor, 0)
  const diff = row.amountMinor - allocatedMinor
  const isBalanced = diff === 0

  return (
    <div className="bg-muted/20 border-t border-border">
      <table className="w-full text-sm table-fixed border-separate border-spacing-0">
        <SplitEditorColGroup />
        <tbody>
          <tr className="bg-muted/40">
            <td colSpan={7} className="px-2 py-1 text-xs font-medium text-muted-foreground">
              {t('import.reviewStep.split.title', { amount: '' })}
              <NumberValue value={row.amountMinor} minorUnits={selectedAccountMinorUnits} currencyCode={selectedAccountCurrencyCode} className="ml-0" />
            </td>
          </tr>
          {legs.map((leg, i) => (
            <SplitLegRow
              key={leg.legId}
              leg={leg}
              legNumber={i + 1}
              selectedAccountMinorUnits={selectedAccountMinorUnits}
              selectedAccountCurrencyCode={selectedAccountCurrencyCode}
              availableBuckets={availableBuckets}
              accountsWithoutIban={accountsWithoutIban}
              counterpartAccounts={counterpartAccounts}
              canRemove={legs.length > 1}
              onAmountChange={(legId, amountMinor) => onLegAmountChange(row.index, legId, amountMinor)}
              onNoteChange={(legId, note) => onLegNoteChange(row.index, legId, note)}
              onPartnerChange={(legId, rawIban, ibanMatch, counterpartAccountId) => onLegPartnerChange(row.index, legId, rawIban, ibanMatch, counterpartAccountId)}
              onBucketChange={(legId, bucketId) => onLegBucketChange(row.index, legId, bucketId)}
              onRemove={(legId) => onRemoveLeg(row.index, legId)}
              onCreatePartner={onCreatePartner}
              onAssignIban={onAssignIban}
            />
          ))}
          <tr className="align-top">
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5 text-xs text-muted-foreground">{t('import.reviewStep.split.remaining')}</td>
            <td className="px-2 py-1.5">
              <div className={cn('flex items-center justify-end gap-1 text-xs', isBalanced ? 'text-green-600 dark:text-green-400' : 'text-destructive')}>
                {isBalanced && <Check className="h-3.5 w-3.5 shrink-0" />}
                <NumberValue value={diff} minorUnits={selectedAccountMinorUnits} currencyCode={selectedAccountCurrencyCode} />
              </div>
            </td>
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5">
              <Button type="button" variant="ghost" size="sm" className="h-7 text-xs whitespace-nowrap px-2" onClick={() => onAddLeg(row.index)}>
                {t('import.reviewStep.split.addSplit')}
              </Button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  )
}
