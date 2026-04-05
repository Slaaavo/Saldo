import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ImportRow } from '../types'
import type { SnapshotRow } from '../../../../shared/types'
import { Button } from '../../../../shared/ui/button'
import { Input } from '../../../../shared/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../../shared/ui/select'

export interface IbanActionCellProps {
  row: ImportRow
  accountsWithoutIban: SnapshotRow[]
  onCreatePartner: (iban: string, name: string) => Promise<void>
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>
  isFirstOccurrence: boolean
}

export const IbanActionCell = ({ row, accountsWithoutIban, onCreatePartner, onAssignIban, isFirstOccurrence }: IbanActionCellProps) => {
  const { t } = useTranslation()
  const [partnerName, setPartnerName] = useState('')
  const [assignAccountId, setAssignAccountId] = useState<string>('')
  const [creating, setCreating] = useState(false)
  const [assigning, setAssigning] = useState(false)

  if (row.ibanMatch.type !== 'unmatched' || !row.rawIban) return null

  if (!isFirstOccurrence) {
    return <span className="text-xs text-muted-foreground italic">↑ resolve above</span>
  }

  return (
    <div className="flex flex-col gap-1 mt-1">
      <div className="flex items-center gap-1">
        <Input
          className="h-7 text-xs px-2 py-1"
          placeholder={t('import.reviewStep.newPartner')}
          value={partnerName}
          onChange={(e) => setPartnerName(e.target.value)}
          disabled={creating}
        />
        <Button
          type="button"
          size="sm"
          className="h-7 text-xs px-2"
          disabled={creating || partnerName.trim() === ''}
          onClick={async () => {
            setCreating(true)
            await onCreatePartner(row.rawIban!, partnerName.trim())
            setCreating(false)
            setPartnerName('')
          }}
        >
          {t('import.reviewStep.createPartner')}
        </Button>
      </div>
      {accountsWithoutIban.length > 0 && (
        <div className="flex items-center gap-1">
          <Select value={assignAccountId} onValueChange={setAssignAccountId} disabled={assigning}>
            <SelectTrigger className="h-7 text-xs">
              <SelectValue placeholder={t('import.reviewStep.assignToAccount')} />
            </SelectTrigger>
            <SelectContent>
              {accountsWithoutIban.map((a) => (
                <SelectItem key={a.accountId} value={String(a.accountId)}>
                  {a.accountName}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            type="button"
            size="sm"
            className="h-7 text-xs px-2"
            disabled={assigning || assignAccountId === ''}
            onClick={async () => {
              setAssigning(true)
              await onAssignIban(row.rawIban!, Number(assignAccountId))
              setAssigning(false)
              setAssignAccountId('')
            }}
          >
            {t('import.reviewStep.assignToAccount')}
          </Button>
        </div>
      )}
    </div>
  )
}
