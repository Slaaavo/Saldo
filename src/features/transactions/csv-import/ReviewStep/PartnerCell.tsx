import { useTranslation } from 'react-i18next'
import { AlertTriangle } from 'lucide-react'
import type { ImportRow } from '../types'
import type { SnapshotRow } from '../../../../shared/types'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'
import { IbanActionCell } from './IbanActionCell'

const COUNTERPART_NONE = '__none__'

export interface PartnerCellProps {
  row: ImportRow
  accountsWithoutIban: SnapshotRow[]
  allAccounts: SnapshotRow[]
  onCreatePartner: (iban: string, name: string) => Promise<void>
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>
  onCounterpartChange: (index: number, accountId: number | null) => void
  isFirstOccurrence: boolean
}

export function PartnerCell({ row, accountsWithoutIban, allAccounts, onCreatePartner, onAssignIban, onCounterpartChange, isFirstOccurrence }: PartnerCellProps) {
  const { t } = useTranslation()

  switch (row.ibanMatch.type) {
    case 'partner':
      return (
        <div className="flex flex-col">
          <span className="text-sm">{row.ibanMatch.accountName}</span>
          {row.rawIban && <span className="text-xs text-muted-foreground truncate max-w-[140px]">{row.rawIban}</span>}
        </div>
      )

    case 'ownAccount':
      return (
        <div className="flex flex-col">
          <div className="flex items-center gap-1">
            <span className="text-sm">{row.ibanMatch.accountName}</span>
            <span className="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300">
              {t('import.reviewStep.transfer')}
            </span>
          </div>
        </div>
      )

    case 'unmatched':
      return (
        <div className="flex flex-col">
          <div className="flex items-center gap-1">
            <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
            <span className="text-xs text-muted-foreground truncate max-w-[120px]">{row.rawIban}</span>
          </div>
          <IbanActionCell row={row} accountsWithoutIban={accountsWithoutIban} onCreatePartner={onCreatePartner} onAssignIban={onAssignIban} isFirstOccurrence={isFirstOccurrence} />
        </div>
      )

    case 'none':
    default:
      return (
        <Select
          value={row.counterpartAccountId !== null ? String(row.counterpartAccountId) : COUNTERPART_NONE}
          onValueChange={(v) => onCounterpartChange(row.index, v === COUNTERPART_NONE ? null : Number(v))}
        >
          <SelectTrigger className="h-7 text-xs max-w-[160px]">
            <SelectValue placeholder={t('import.reviewStep.assignCounterpart')} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={COUNTERPART_NONE}>—</SelectItem>
            {allAccounts.map((a) => (
              <SelectItem key={a.accountId} value={String(a.accountId)}>
                {a.accountName}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )
  }
}
