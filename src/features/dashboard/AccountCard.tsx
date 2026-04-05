import { useTranslation } from 'react-i18next'
import type { SnapshotRow, Currency } from '../../shared/types'
import NumberValue from '../../shared/ui/NumberValue'
import BucketAmountWithTooltip from '../buckets/BucketAmountWithTooltip'
import { formatAmount } from '../../shared/utils/format'
import { defaultNumberFormat } from '../../shared/config/numberFormat'
import { cn } from '@/shared/lib/utils'
import { Button } from '../../shared/ui/button'
import { Card, CardContent } from '../../shared/ui/card'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../../shared/ui/tooltip'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../../shared/ui/dropdown-menu'
import { MoreVertical, Pencil, Trash2, Link2 } from 'lucide-react'
import { formatIbanSegments } from '../../shared/utils/formatIban'

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
}: AccountCardProps) => {
  const { t } = useTranslation()

  const numConfig = { ...defaultNumberFormat, currencySymbol: '' }
  const fmtNum = (amountMinor: number, minorUnits: number) => formatAmount(amountMinor, minorUnits, numConfig).trim()

  const hasEquityTooltip = row.accountType === 'asset' && row.linkedAssetIds.length > 0 && !!allAccounts && !!consolidationCurrency
  const equityLinkedRows = hasEquityTooltip ? allAccounts!.filter((a) => row.linkedAssetIds.includes(a.accountId)) : []
  const equityMinor = hasEquityTooltip ? row.convertedBalanceMinor + equityLinkedRows.reduce((sum, a) => sum + a.convertedBalanceMinor, 0) : 0

  return (
    <Card className={cn('relative w-[250px] min-w-[250px] shrink-0')}>
      <CardContent className="flex flex-col gap-1 p-4">
        <div className="flex items-start justify-between min-w-0">
          <div className="flex items-center gap-1 min-w-0">
            {row.isLinkedToAsset && allAssets && (
              <TooltipProvider delayDuration={300}>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="shrink-0 cursor-help">
                      <Link2 className="h-3.5 w-3.5 text-muted-foreground" />
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p className="text-xs">
                      {t('accounts.linkedAssetTooltip', {
                        assets: allAssets
                          .filter((a) => row.linkedAssetIds.includes(a.accountId))
                          .map((a) => a.accountName)
                          .join(', '),
                      })}
                    </p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            )}
            <span className="text-sm text-muted-foreground truncate" title={row.accountName}>
              {row.accountName}
            </span>
          </div>
          <div className="flex items-center">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="h-7 w-7 -mr-2 -mt-1">
                  <MoreVertical className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={() => onRenameAccount(row.accountId, row.accountName)}>
                  <Pencil className="h-4 w-4" />
                  {row.accountType === 'account' ? t('accounts.edit') : t('accounts.rename')}
                </DropdownMenuItem>
                {onManageLinkedAssets && row.accountType === 'account' && (
                  <DropdownMenuItem onClick={() => onManageLinkedAssets(row.accountId, row.accountName)}>
                    <Link2 className="h-4 w-4" />
                    {t('accounts.manageLinkedAssets')}
                  </DropdownMenuItem>
                )}
                <DropdownMenuItem className="text-destructive focus:text-destructive" onClick={() => onDeleteAccount(row.accountId, row.accountName)}>
                  <Trash2 className="h-4 w-4" />
                  {t('accounts.delete')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
        {row.accountType === 'account' &&
          (row.iban?.trim() ? (
            <p className="text-xs text-muted-foreground truncate" title={row.iban.replace(/(.{4})/g, '$1 ').trim()}>
              {formatIbanSegments(row.iban).map((seg, i) => (
                <span key={i} className={seg.weight}>
                  {seg.text}
                </span>
              ))}
            </p>
          ) : (
            <span className="text-xs text-muted-foreground">{t('accounts.noIban')}</span>
          ))}
        {row.accountType === 'bucket' ? (
          <BucketAmountWithTooltip
            totalMinor={row.convertedBalanceMinor}
            manualBalanceMinor={row.balanceMinor}
            bucketLinks={row.bucketLinks}
            currencyCode={consolidationCurrency?.code ?? row.currencyCode}
            minorUnits={consolidationCurrency?.minorUnits ?? row.currencyMinorUnits}
            manualCurrencyCode={row.currencyCode}
            manualMinorUnits={row.currencyMinorUnits}
            allAccounts={allAccounts}
            consolidationCurrencyCode={consolidationCurrency?.code ?? row.currencyCode}
            cashflowTaggedMinor={row.cashflowTaggedMinor}
            className={cn('text-2xl font-bold', row.convertedBalanceMinor < 0 && 'text-destructive')}
          />
        ) : row.isCustom && consolidationCurrency ? (
          // Unit-denominated asset: show consolidated value as primary, tooltip with breakdown
          <TooltipProvider delayDuration={300}>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="cursor-help inline-block">
                  <NumberValue
                    value={row.convertedBalanceMinor}
                    currencyCode={consolidationCurrency.code}
                    minorUnits={consolidationCurrency.minorUnits}
                    className={cn('text-2xl font-bold', row.convertedBalanceMinor < 0 && 'text-destructive')}
                  />
                </span>
              </TooltipTrigger>
              <TooltipContent>
                <p className="text-xs">
                  {`${fmtNum(row.balanceMinor, row.currencyMinorUnits)} ${row.currencyCode} = ${formatAmount(row.convertedBalanceMinor, consolidationCurrency.minorUnits, defaultNumberFormat, consolidationCurrency.code)}`}
                </p>
                {row.fxRateMissing && <p className="text-xs text-muted-foreground mt-1">{t('accounts.fxRateMissingTooltip')}</p>}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        ) : consolidationCurrency && row.currencyCode !== consolidationCurrency.code ? (
          <TooltipProvider delayDuration={300}>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="cursor-help inline-block">
                  <NumberValue
                    value={row.balanceMinor}
                    currencyCode={row.currencyCode}
                    minorUnits={row.currencyMinorUnits}
                    className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
                  />
                </span>
              </TooltipTrigger>
              <TooltipContent>
                <p className="text-xs">{`≈ ${formatAmount(row.convertedBalanceMinor, consolidationCurrency.minorUnits, defaultNumberFormat, consolidationCurrency.code)}`}</p>
                {row.fxRateMissing && <p className="text-xs text-muted-foreground mt-1">{t('accounts.fxRateMissingTooltip')}</p>}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        ) : (
          <NumberValue
            value={row.balanceMinor}
            currencyCode={row.currencyCode}
            minorUnits={row.currencyMinorUnits}
            className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
          />
        )}
        {hasEquityTooltip && (
          <TooltipProvider delayDuration={300}>
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="cursor-help mt-1 text-xs text-muted-foreground inline-flex items-center gap-1">
                  {t('assets.equity')}:{' '}
                  <NumberValue
                    value={equityMinor}
                    currencyCode={consolidationCurrency!.code}
                    minorUnits={consolidationCurrency!.minorUnits}
                    className="font-medium text-foreground"
                  />
                </span>
              </TooltipTrigger>
              <TooltipContent>
                <div className="text-xs space-y-1 min-w-[160px]">
                  <div className="flex justify-between gap-4">
                    <span>{row.accountName}</span>
                    <NumberValue value={row.convertedBalanceMinor} currencyCode={consolidationCurrency!.code} minorUnits={consolidationCurrency!.minorUnits} />
                  </div>
                  {equityLinkedRows.map((a) => (
                    <div key={a.accountId} className="flex justify-between gap-4">
                      <span className="text-muted-foreground">{a.accountName}</span>
                      <NumberValue value={a.convertedBalanceMinor} currencyCode={consolidationCurrency!.code} minorUnits={consolidationCurrency!.minorUnits} />
                    </div>
                  ))}
                  <div className="flex justify-between gap-4 border-t border-border pt-1 font-medium">
                    <span>{t('assets.equity')}</span>
                    <NumberValue value={equityMinor} currencyCode={consolidationCurrency!.code} minorUnits={consolidationCurrency!.minorUnits} />
                  </div>
                </div>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        )}
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
