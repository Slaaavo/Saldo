import { useTranslation } from 'react-i18next'
import type { SnapshotRow, Currency } from '@/shared/types'
import { cn } from '@/shared/lib/utils'
import { takesEntries } from '@/shared/utils/snapshot'
import { Card, CardContent } from '@/shared/ui/card'
import { Pencil } from 'lucide-react'
import BucketSwitch from '../../buckets/BucketSwitch'
import { BalanceDisplay } from './BalanceDisplay'
import { BankCheckButtons } from './BankCheckButtons'
import { EquityTooltip } from './EquityTooltip'
import { AccountContextMenu } from './AccountContextMenu'
import { IbanDisplay } from './IbanDisplay'
import { LinkedAssetIcon } from './LinkedAssetIcon'
import { VerificationLine } from './VerificationLine'
import { useAccountCardEquity } from './useAccountCardEquity'

interface AccountCardProps {
  row: SnapshotRow
  consolidationCurrency?: Currency | null
  updateButtonLabel?: string
  onUpdateBalance: (accountId: number) => void
  onEditIncomeBudget?: (bucketId: number) => void
  onRenameAccount: (accountId: number, currentName: string) => void
  onDeleteAccount: (accountId: number, name: string) => void
  onManageLinkedAssets?: (accountId: number, accountName: string) => void
  onViewInLedger?: (accountId: number) => void
  /** Given only in live mode. An own account's card then offers Matches bank and Find the difference. */
  onFindBankDifference?: (accountId: number) => void
  allAssets?: SnapshotRow[]
  allAccounts?: SnapshotRow[]
  personName?: string
}

const AccountCard = ({
  row,
  consolidationCurrency,
  updateButtonLabel,
  onUpdateBalance,
  onEditIncomeBudget,
  onRenameAccount,
  onDeleteAccount,
  onManageLinkedAssets,
  onViewInLedger,
  onFindBankDifference,
  allAssets,
  allAccounts,
  personName,
}: AccountCardProps) => {
  const { t } = useTranslation()

  const equity = useAccountCardEquity(row, allAccounts)
  const hasEntries = takesEntries(row)
  const action = hasEntries
    ? { label: updateButtonLabel ?? t('accounts.updateBalance'), run: () => onUpdateBalance(row.accountId) }
    : onEditIncomeBudget && { label: t('incomeBudget.edit'), run: () => onEditIncomeBudget(row.accountId) }
  const findBankDifference = row.accountType === 'account' ? onFindBankDifference : undefined

  return (
    <Card className={cn('relative w-[250px] min-w-[250px] shrink-0')}>
      <CardContent className="flex flex-col gap-1 p-4">
        <div className="flex items-start justify-between min-w-0">
          <div className="flex items-center gap-1 min-w-0">
            {row.isLinkedToAsset && allAssets && <LinkedAssetIcon linkedAssetIds={row.linkedAssetIds} allAssets={allAssets} />}
            <span className="text-sm text-muted-foreground truncate" title={row.accountName}>
              {row.accountName}
            </span>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            {row.bucket && <BucketSwitch bucketId={row.accountId} bucketName={row.accountName} enabled={row.bucket.enabled} />}
            <AccountContextMenu
              accountId={row.accountId}
              accountName={row.accountName}
              accountType={row.accountType}
              onRenameAccount={onRenameAccount}
              onDeleteAccount={onDeleteAccount}
              onManageLinkedAssets={onManageLinkedAssets}
              onViewInLedger={hasEntries ? onViewInLedger : undefined}
            />
          </div>
        </div>
        {personName && <span className="text-xs text-muted-foreground">{personName}</span>}
        {row.accountType === 'account' && <IbanDisplay iban={row.iban} />}
        <BalanceDisplay row={row} consolidationCurrency={consolidationCurrency} />
        <VerificationLine row={row} />
        {equity && consolidationCurrency && (
          <EquityTooltip row={row} equityMinor={equity.equityMinor} equityLinkedRows={equity.equityLinkedRows} consolidationCurrency={consolidationCurrency} />
        )}
        {(action || findBankDifference) && (
          <div className="mt-2 flex items-center justify-between gap-1">
            {action && (
              <button
                onClick={action.run}
                className="min-w-0 flex-1 text-xs font-bold uppercase tracking-wider text-muted-foreground hover:text-primary transition-colors flex items-center gap-1"
              >
                <Pencil className="h-3 w-3 shrink-0" />
                <span className="truncate">{action.label}</span>
              </button>
            )}
            {findBankDifference && <BankCheckButtons row={row} onFindDifference={findBankDifference} />}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

export default AccountCard
