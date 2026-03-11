import { useRef, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { SnapshotRow, Currency } from '../types';
import NumberValue from './NumberValue';
import BucketAmountWithTooltip from './BucketAmountWithTooltip';
import { formatAmount } from '../utils/format';
import { defaultNumberFormat } from '../config/numberFormat';
import { cn } from '@/lib/utils';
import { Button } from './ui/button';
import { Card, CardContent } from './ui/card';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';
import {
  MoreVertical,
  Plus,
  Pencil,
  Trash2,
  ChevronLeft,
  ChevronRight,
  AlertTriangle,
  ArrowUpDown,
  Link2,
} from 'lucide-react';

interface Props {
  snapshot: SnapshotRow[];
  consolidationCurrency?: Currency | null;
  sectionTitle?: string;
  addButtonLabel?: string;
  updateButtonLabel?: string;
  emptyMessage?: string;
  onUpdateBalance: (accountId: number) => void;
  onRenameAccount: (accountId: number, currentName: string) => void;
  onDeleteAccount: (accountId: number, name: string) => void;
  onCreateAccount: () => void;
  onReorder?: () => void;
  onManageLinkedAssets?: (accountId: number, accountName: string) => void;
  /** All asset rows — used to resolve asset names for the link indicator tooltip on account cards */
  allAssets?: SnapshotRow[];
  /** All account rows — used to resolve linked account balances for equity tooltip on asset cards */
  allAccounts?: SnapshotRow[];
}

export default function AccountCards({
  snapshot,
  consolidationCurrency,
  sectionTitle,
  addButtonLabel,
  updateButtonLabel,
  emptyMessage,
  onUpdateBalance,
  onRenameAccount,
  onDeleteAccount,
  onCreateAccount,
  onReorder,
  onManageLinkedAssets,
  allAssets,
  allAccounts,
}: Props) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const updateScrollButtons = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 0);
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  }, []);

  useEffect(() => {
    updateScrollButtons();
    const el = scrollRef.current;
    if (!el) return;
    el.addEventListener('scroll', updateScrollButtons, { passive: true });
    const ro = new ResizeObserver(updateScrollButtons);
    ro.observe(el);
    return () => {
      el.removeEventListener('scroll', updateScrollButtons);
      ro.disconnect();
    };
  }, [updateScrollButtons, snapshot]);

  const scroll = useCallback((direction: 'left' | 'right') => {
    const el = scrollRef.current;
    if (!el) return;
    const amount = direction === 'left' ? -270 : 270;
    el.scrollBy({ left: amount, behavior: 'smooth' });
  }, []);

  const numConfig = { ...defaultNumberFormat, currencySymbol: '' };
  const fmtNum = (amountMinor: number, minorUnits: number) =>
    formatAmount(amountMinor, minorUnits, numConfig).trim();

  return (
    <section>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold">{sectionTitle ?? t('accounts.sectionTitle')}</h2>
        <div className="flex items-center gap-2">
          {onReorder && snapshot.length > 1 && (
            <Button onClick={onReorder} size="sm" variant="ghost">
              <ArrowUpDown className="h-4 w-4" />
            </Button>
          )}
          <Button onClick={onCreateAccount} size="sm" variant="outline">
            <Plus className="h-4 w-4" />
            {addButtonLabel ?? t('accounts.addAccount')}
          </Button>
        </div>
      </div>

      {snapshot.length === 0 ? (
        <p className="text-sm text-muted-foreground">{emptyMessage ?? t('accounts.empty')}</p>
      ) : (
        <div className="group/scroll relative">
          {canScrollLeft && (
            <button
              onClick={() => scroll('left')}
              className="absolute left-0 top-1/2 -translate-y-1/2 z-10 hidden group-hover/scroll:flex h-8 w-8 items-center justify-center rounded-full bg-background border border-border shadow-md hover:bg-accent transition-colors"
              aria-label={t('accounts.scrollLeft')}
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
          )}

          <div
            ref={scrollRef}
            className="flex gap-4 overflow-x-auto no-scrollbar"
            style={{ scrollbarWidth: 'none' }}
          >
            {snapshot.map((row) => {
              const isOverAllocated =
                row.accountType === 'account' &&
                row.allocatedTotalMinor > 0 &&
                row.allocatedTotalMinor > row.balanceMinor;
              const overAllocationTitle = isOverAllocated
                ? t('accounts.overAllocationTooltip', {
                    allocated: fmtNum(row.allocatedTotalMinor, row.currencyMinorUnits),
                    balance: fmtNum(row.balanceMinor, row.currencyMinorUnits),
                    currency: row.currencyCode,
                    over: fmtNum(
                      row.allocatedTotalMinor - row.balanceMinor,
                      row.currencyMinorUnits,
                    ),
                    buckets: row.overAllocationBuckets.map((b) => b.bucketName).join(', '),
                  })
                : undefined;
              const hasEquityTooltip =
                row.accountType === 'asset' &&
                row.linkedAssetIds.length > 0 &&
                !!allAccounts &&
                !!consolidationCurrency;
              const equityLinkedRows = hasEquityTooltip
                ? allAccounts!.filter((a) => row.linkedAssetIds.includes(a.accountId))
                : [];
              const equityMinor = hasEquityTooltip
                ? row.convertedBalanceMinor +
                  equityLinkedRows.reduce((sum, a) => sum + a.convertedBalanceMinor, 0)
                : 0;
              return (
                <Card
                  key={row.accountId}
                  className={cn(
                    'relative w-[250px] min-w-[250px] shrink-0',
                    isOverAllocated && 'border-amber-500',
                  )}
                >
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
                        <span
                          className="text-sm text-muted-foreground truncate"
                          title={row.accountName}
                        >
                          {row.accountName}
                        </span>
                      </div>
                      <div className="flex items-center">
                        {isOverAllocated && (
                          <span title={overAllocationTitle}>
                            <AlertTriangle className="h-4 w-4 text-amber-500 shrink-0" />
                          </span>
                        )}
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button variant="ghost" size="icon" className="h-7 w-7 -mr-2 -mt-1">
                              <MoreVertical className="h-4 w-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem
                              onClick={() => onRenameAccount(row.accountId, row.accountName)}
                            >
                              <Pencil className="h-4 w-4" />
                              {t('accounts.rename')}
                            </DropdownMenuItem>
                            {onManageLinkedAssets && row.accountType === 'account' && (
                              <DropdownMenuItem
                                onClick={() => onManageLinkedAssets(row.accountId, row.accountName)}
                              >
                                <Link2 className="h-4 w-4" />
                                {t('accounts.manageLinkedAssets')}
                              </DropdownMenuItem>
                            )}
                            <DropdownMenuItem
                              className="text-destructive focus:text-destructive"
                              onClick={() => onDeleteAccount(row.accountId, row.accountName)}
                            >
                              <Trash2 className="h-4 w-4" />
                              {t('accounts.delete')}
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>
                    </div>
                    {row.accountType === 'bucket' ? (
                      <BucketAmountWithTooltip
                        totalMinor={row.convertedBalanceMinor}
                        manualBalanceMinor={row.balanceMinor}
                        allocations={row.linkedAllocations}
                        currencyCode={consolidationCurrency?.code ?? row.currencyCode}
                        minorUnits={consolidationCurrency?.minorUnits ?? row.currencyMinorUnits}
                        manualCurrencyCode={row.currencyCode}
                        manualMinorUnits={row.currencyMinorUnits}
                        className={cn(
                          'text-2xl font-bold',
                          row.convertedBalanceMinor < 0 && 'text-destructive',
                        )}
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
                                className={cn(
                                  'text-2xl font-bold',
                                  row.convertedBalanceMinor < 0 && 'text-destructive',
                                )}
                              />
                            </span>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p className="text-xs">
                              {`${fmtNum(row.balanceMinor, row.currencyMinorUnits)} ${row.currencyCode} = ${formatAmount(row.convertedBalanceMinor, consolidationCurrency.minorUnits, defaultNumberFormat, consolidationCurrency.code)}`}
                            </p>
                            {row.fxRateMissing && (
                              <p className="text-xs text-muted-foreground mt-1">
                                {t('accounts.fxRateMissingTooltip')}
                              </p>
                            )}
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
                                className={cn(
                                  'text-2xl font-bold',
                                  row.balanceMinor < 0 && 'text-destructive',
                                )}
                              />
                            </span>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p className="text-xs">
                              {`≈ ${formatAmount(row.convertedBalanceMinor, consolidationCurrency.minorUnits, defaultNumberFormat, consolidationCurrency.code)}`}
                            </p>
                            {row.fxRateMissing && (
                              <p className="text-xs text-muted-foreground mt-1">
                                {t('accounts.fxRateMissingTooltip')}
                              </p>
                            )}
                          </TooltipContent>
                        </Tooltip>
                      </TooltipProvider>
                    ) : (
                      <NumberValue
                        value={row.balanceMinor}
                        currencyCode={row.currencyCode}
                        minorUnits={row.currencyMinorUnits}
                        className={cn(
                          'text-2xl font-bold',
                          row.balanceMinor < 0 && 'text-destructive',
                        )}
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
                                <NumberValue
                                  value={row.convertedBalanceMinor}
                                  currencyCode={consolidationCurrency!.code}
                                  minorUnits={consolidationCurrency!.minorUnits}
                                />
                              </div>
                              {equityLinkedRows.map((a) => (
                                <div key={a.accountId} className="flex justify-between gap-4">
                                  <span className="text-muted-foreground">{a.accountName}</span>
                                  <NumberValue
                                    value={a.convertedBalanceMinor}
                                    currencyCode={consolidationCurrency!.code}
                                    minorUnits={consolidationCurrency!.minorUnits}
                                  />
                                </div>
                              ))}
                              <div className="flex justify-between gap-4 border-t border-border pt-1 font-medium">
                                <span>{t('assets.equity')}</span>
                                <NumberValue
                                  value={equityMinor}
                                  currencyCode={consolidationCurrency!.code}
                                  minorUnits={consolidationCurrency!.minorUnits}
                                />
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
              );
            })}
          </div>

          {canScrollRight && (
            <button
              onClick={() => scroll('right')}
              className="absolute right-0 top-1/2 -translate-y-1/2 z-10 hidden group-hover/scroll:flex h-8 w-8 items-center justify-center rounded-full bg-background border border-border shadow-md hover:bg-accent transition-colors"
              aria-label={t('accounts.scrollRight')}
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          )}
        </div>
      )}
    </section>
  );
}
