import { useTranslation } from 'react-i18next'
import { CheckCheck, Scale } from 'lucide-react'
import type { SnapshotRow } from '@/shared/types'
import { Button } from '@/shared/ui/button'
import { useConfirmBankMatch } from '../../bank/useBankCheckMutations'

interface Props {
  row: SnapshotRow
  onFindDifference: (accountId: number) => void
}

export const BankCheckButtons = ({ row, onFindDifference }: Props) => {
  const { t } = useTranslation()
  const confirmMatch = useConfirmBankMatch()

  return (
    <div className="-my-1.5 -mr-2 flex shrink-0 items-center">
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7"
        title={t('bankCheck.matchesBank')}
        aria-label={t('bankCheck.matchesBank')}
        disabled={confirmMatch.isPending}
        onClick={() => confirmMatch.mutate({ accountId: row.accountId, seenBalanceMinor: row.balanceMinor })}
      >
        <CheckCheck className="h-4 w-4" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7"
        title={t('bankCheck.findDifference')}
        aria-label={t('bankCheck.findDifference')}
        onClick={() => onFindDifference(row.accountId)}
      >
        <Scale className="h-4 w-4" />
      </Button>
    </div>
  )
}
