import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { X } from 'lucide-react'
import type { SplitLeg, IbanMatchResult, ImportRow } from '../types'
import type { SnapshotRow } from '../../../../shared/types'
import { fromMinorUnits, toMinorUnits } from '../../../../shared/utils/format'
import { Button } from '../../../../shared/ui/button'
import { Input } from '../../../../shared/ui/input'
import { CurrencyInput } from '../../../../shared/ui/CurrencyInput'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'
import { PartnerCell } from './PartnerCell'

const BUCKET_NONE = '__none__'

interface SplitLegRowProps {
  leg: SplitLeg
  legNumber: number
  selectedAccountMinorUnits: number
  selectedAccountCurrencyCode: string
  availableBuckets: SnapshotRow[]
  accountsWithoutIban: SnapshotRow[]
  allAccounts: SnapshotRow[]
  canRemove: boolean
  onAmountChange: (legIndex: number, amountMinor: number) => void
  onNoteChange: (legIndex: number, note: string | null) => void
  onPartnerChange: (legIndex: number, rawIban: string | null, ibanMatch: IbanMatchResult, counterpartAccountId: number | null) => void
  onBucketChange: (legIndex: number, bucketId: number | null) => void
  onRemove: (legIndex: number) => void
  onCreatePartner: (iban: string, name: string) => Promise<void>
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>
}

export function SplitLegRow({
  leg,
  legNumber,
  selectedAccountMinorUnits,
  selectedAccountCurrencyCode,
  availableBuckets,
  accountsWithoutIban,
  allAccounts,
  canRemove,
  onAmountChange,
  onNoteChange,
  onPartnerChange,
  onBucketChange,
  onRemove,
  onCreatePartner,
  onAssignIban,
}: SplitLegRowProps) {
  const { t } = useTranslation()

  const [amountStr, setAmountStr] = useState(() => fromMinorUnits(leg.amountMinor, selectedAccountMinorUnits))

  // Construct a synthetic ImportRow so we can reuse PartnerCell unchanged
  const syntheticRow: ImportRow = {
    index: leg.legIndex,
    date: '',
    amountMinor: leg.amountMinor,
    currencyCode: selectedAccountCurrencyCode,
    originalAmountMinor: null,
    originalCurrencyCode: null,
    fxRateMantissa: null,
    fxRateExponent: null,
    note: leg.note,
    rawIban: leg.rawIban,
    ibanMatch: leg.ibanMatch,
    isDuplicate: false,
    nearDateDuplicateEventId: null,
    isSelected: true,
    bucketId: leg.bucketId,
    counterpartAccountId: leg.counterpartAccountId,
    splitLegs: null,
  }

  const handleCounterpartChange = (_index: number, accountId: number | null) => {
    if (accountId === null) {
      onPartnerChange(leg.legIndex, null, { type: 'none' }, null)
      return
    }
    const account = allAccounts.find((a) => a.accountId === accountId)
    if (!account) return
    const ibanMatch: IbanMatchResult =
      account.accountType === 'account' ? { type: 'ownAccount', accountId, accountName: account.accountName } : { type: 'partner', accountId, accountName: account.accountName }
    onPartnerChange(leg.legIndex, null, ibanMatch, accountId)
  }

  return (
    <tr className="align-top">
      <td className="px-2 py-1.5 border-b border-border/50" />
      <td className="px-2 py-1.5 border-b border-border/50 text-xs text-muted-foreground whitespace-nowrap">{t('import.reviewStep.split.splitLabel', { n: legNumber })}</td>
      <td className="px-2 py-1.5 border-b border-border/50">
        <CurrencyInput
          type="text"
          inputMode="decimal"
          className="h-7 text-xs text-right"
          value={amountStr}
          currencyCode={selectedAccountCurrencyCode}
          onChange={(e) => setAmountStr(e.target.value)}
          onBlur={() => {
            const parsed = toMinorUnits(amountStr, selectedAccountMinorUnits)
            onAmountChange(leg.legIndex, parsed)
            setAmountStr(fromMinorUnits(parsed, selectedAccountMinorUnits))
          }}
        />
      </td>
      <td className="px-2 py-1.5 border-b border-border/50">
        <PartnerCell
          row={syntheticRow}
          accountsWithoutIban={accountsWithoutIban}
          allAccounts={allAccounts}
          onCreatePartner={onCreatePartner}
          onAssignIban={onAssignIban}
          onCounterpartChange={handleCounterpartChange}
          isFirstOccurrence={true}
        />
      </td>
      <td className="px-2 py-1.5 border-b border-border/50">
        <Input className="h-7 text-xs" value={leg.note ?? ''} onChange={(e) => onNoteChange(leg.legIndex, e.target.value || null)} />
      </td>
      <td className="px-2 py-1.5 border-b border-border/50">
        <Select value={leg.bucketId !== null ? String(leg.bucketId) : BUCKET_NONE} onValueChange={(v) => onBucketChange(leg.legIndex, v === BUCKET_NONE ? null : Number(v))}>
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
      </td>
      <td className="px-2 py-1.5 border-b border-border/50">
        {canRemove && (
          <Button type="button" variant="ghost" size="sm" className="h-7 w-7 p-0" onClick={() => onRemove(leg.legIndex)}>
            <X className="h-3.5 w-3.5" />
          </Button>
        )}
      </td>
    </tr>
  )
}
