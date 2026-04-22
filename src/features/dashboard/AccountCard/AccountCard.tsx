import { useTranslation } from 'react-i18next'
import type { SnapshotRow, Currency } from '@/shared/types'
import { cn } from '@/shared/lib/utils'
import { Card, CardContent } from '@/shared/ui/card'
import { Pencil } from 'lucide-react'
import { BalanceDisplay } from './BalanceDisplay'
import { EquityTooltip } from './EquityTooltip'
import { AccountContextMenu } from './AccountContextMenu'
import { IbanDisplay } from './IbanDisplay'
import { LinkedAssetIcon } from './LinkedAssetIcon'
import { useAccountCardEquity } from './useAccountCardEquity'

interface AccountCardProps {
  row: SnapshotRow
  consolidationCurrency?: Currency | null
  updateButtonLabel?: string
  onUpdateBalance: (accountId: number) => void
  onRenameAccount: (accountId: number, currentName: string) => void
  onDeleteAccount: (accountId: number, name: string) => void
  onManageLinkedAssets?: (accountId: number, accountName: string) => void
  allAssets?: SnapshotRow[]
  allAccounts?: SnapshotRow[]
  personName?: string
}

const AccountCard = ({
  row,
  consolidationCurrency,
  updateButtonLabel,
  onUpdateBalance,
  onRenameAccount,
  onDeleteAccount,
  onManageLinkedAssets,
  allAssets,
  allAccounts,
  personName,
}: AccountCardProps) => {
  const { t } = useTranslation()

  const { hasEquityTooltip, equityLinkedRows, equityMinor } = useAccountCardEquity(row, allAccounts, consolidationCurrency)

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
          <AccountContextMenu
            accountId={row.accountId}
            accountName={row.accountName}
            accountType={row.accountType}
            onRenameAccount={onRenameAccount}
            onDeleteAccount={onDeleteAccount}
            onManageLinkedAssets={onManageLinkedAssets}
          />
        </div>
        {personName && <span className="text-xs text-muted-foreground">{personName}</span>}
        {row.accountType === 'account' && <IbanDisplay iban={row.iban} />}
        <BalanceDisplay row={row} consolidationCurrency={consolidationCurrency} allAccounts={allAccounts} />
        {hasEquityTooltip && <EquityTooltip row={row} equityMinor={equityMinor} equityLinkedRows={equityLinkedRows} consolidationCurrency={consolidationCurrency!} />}
        <button
          onClick={() => onUpdateBalance(row.accountId)}
          className="mt-2 text-xs font-bold uppercase tracking-wider text-muted-foreground hover:text-primary transition-colors flex items-center gap-1"
        >
          <Pencil className="h-3 w-3" />
          {updateButtonLabel ?? t('accounts.updateBalance')}
        </button>
      </CardContent>
    </Card>
  )
}

export default AccountCard
