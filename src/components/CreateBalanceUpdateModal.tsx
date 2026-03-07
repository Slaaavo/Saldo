import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../types';
import { todayIso } from '../utils/format';
import { createBucketAllocation, getAccountAllocatedTotal, listBucketAllocations } from '../api';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { DatePicker } from './ui/date-picker';

interface AllocationRow {
  tempId: string;
  sourceAccountId: number | null;
  amountStr: string;
  originalAmountMinor: number;
  isNew: boolean;
  isUnlinked: boolean;
}

interface Props {
  accounts: SnapshotRow[];
  preselectedAccountId?: number;
  onSubmit: (accountId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

let tempIdCounter = 0;
const nextTempId = () => `alloc-${tempIdCounter++}`;

export default function CreateBalanceUpdateModal({
  accounts,
  preselectedAccountId,
  onSubmit,
  onClose,
}: Props) {
  const { t } = useTranslation();

  const initialAccountId = preselectedAccountId ?? accounts[0]?.accountId ?? 0;
  const [accountId, setAccountId] = useState<number>(initialAccountId);
  const [amount, setAmount] = useState(() => {
    const a = accounts.find((acc) => acc.accountId === initialAccountId);
    return a?.accountType === 'bucket' ? '0' : '';
  });
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState('');

  const selectedAccount = accounts.find((a) => a.accountId === accountId);
  const minorUnits = selectedAccount?.currencyMinorUnits ?? 2;
  const currencyCode = selectedAccount?.currencyCode;
  const isBucket = selectedAccount?.accountType === 'bucket';

  const realAccounts = accounts.filter((a) => a.accountType === 'account');

  const [allocations, setAllocations] = useState<AllocationRow[]>([]);
  const [allocatedTotals, setAllocatedTotals] = useState<Map<number, number>>(new Map());
  const [loadingAllocations, setLoadingAllocations] = useState(false);
  const [backendErrors, setBackendErrors] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);

  const loadAllocations = useCallback(async (bucketId: number, asOfDate: string) => {
    setLoadingAllocations(true);
    setAllocations([]);
    setAllocatedTotals(new Map());
    setBackendErrors({});
    try {
      const existing = await listBucketAllocations(bucketId, asOfDate);
      const totalsMap = new Map<number, number>();
      await Promise.all(
        existing.map(async (alloc) => {
          const total = await getAccountAllocatedTotal(alloc.sourceAccountId, asOfDate);
          totalsMap.set(alloc.sourceAccountId, total);
        }),
      );
      setAllocatedTotals(totalsMap);
      setAllocations(
        existing.map((alloc) => ({
          tempId: nextTempId(),
          sourceAccountId: alloc.sourceAccountId,
          amountStr: (alloc.amountMinor / Math.pow(10, alloc.sourceCurrencyMinorUnits)).toFixed(
            alloc.sourceCurrencyMinorUnits,
          ),
          originalAmountMinor: alloc.amountMinor,
          isNew: false,
          isUnlinked: false,
        })),
      );
    } catch (err) {
      console.error('Failed to load bucket allocations:', err);
    } finally {
      setLoadingAllocations(false);
    }
  }, []);

  useEffect(() => {
    if (isBucket && accountId) {
      void loadAllocations(accountId, date);
    } else {
      setAllocations([]);
      setAllocatedTotals(new Map());
      setBackendErrors({});
    }
  }, [accountId, date, isBucket, loadAllocations]);

  const handleAccountChange = (val: string) => {
    const newId = Number(val);
    setAccountId(newId);
    const newAccount = accounts.find((a) => a.accountId === newId);
    setAmount(newAccount?.accountType === 'bucket' ? '0' : '');
  };

  const getAvailableBalance = (row: AllocationRow): number => {
    if (row.sourceAccountId === null) return 0;
    const sourceAccount = realAccounts.find((a) => a.accountId === row.sourceAccountId);
    if (!sourceAccount) return 0;
    const allocated = allocatedTotals.get(row.sourceAccountId) ?? 0;
    return sourceAccount.balanceMinor - allocated + row.originalAmountMinor;
  };

  const handleSourceAccountSelect = async (tempId: string, newSourceAccountId: number) => {
    setAllocations((prev) =>
      prev.map((row) =>
        row.tempId === tempId ? { ...row, sourceAccountId: newSourceAccountId } : row,
      ),
    );
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
    if (!allocatedTotals.has(newSourceAccountId)) {
      try {
        const total = await getAccountAllocatedTotal(newSourceAccountId, date);
        setAllocatedTotals((prev) => new Map(prev).set(newSourceAccountId, total));
      } catch (err) {
        console.error('Failed to get allocated total:', err);
      }
    }
  };

  const handleAddAllocation = () => {
    setAllocations((prev) => [
      ...prev,
      {
        tempId: nextTempId(),
        sourceAccountId: null,
        amountStr: '',
        originalAmountMinor: 0,
        isNew: true,
        isUnlinked: false,
      },
    ]);
  };

  const handleUnlink = (tempId: string) => {
    setAllocations((prev) =>
      prev.map((row) => (row.tempId === tempId ? { ...row, isUnlinked: true } : row)),
    );
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
  };

  const handleRemoveNew = (tempId: string) => {
    setAllocations((prev) => prev.filter((row) => row.tempId !== tempId));
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
  };

  const handleAllocationAmountChange = (tempId: string, value: string) => {
    setAllocations((prev) =>
      prev.map((row) => (row.tempId === tempId ? { ...row, amountStr: value } : row)),
    );
    setBackendErrors((prev) => {
      const next = { ...prev };
      delete next[tempId];
      return next;
    });
  };

  // Compute client-side validation errors live for display and to disable the submit button
  const clientErrors: Record<string, string> = {};
  if (isBucket) {
    for (const row of allocations) {
      if (row.isUnlinked || row.sourceAccountId === null) continue;
      const sourceAccount = realAccounts.find((a) => a.accountId === row.sourceAccountId);
      const sourceMinorUnits = sourceAccount?.currencyMinorUnits ?? 2;
      const parsed = parseFloat(row.amountStr);
      if (row.amountStr !== '' && (isNaN(parsed) || parsed < 0)) {
        clientErrors[row.tempId] = t('validation.invalidAmount');
        continue;
      }
      if (!isNaN(parsed) && parsed >= 0) {
        const amountMinor = Math.round(parsed * Math.pow(10, sourceMinorUnits));
        const available = getAvailableBalance(row);
        if (amountMinor > available) {
          const availableDecimal = (available / Math.pow(10, sourceMinorUnits)).toFixed(
            sourceMinorUnits,
          );
          clientErrors[row.tempId] = t('modals.createBalanceUpdate.exceedsAvailable', {
            amount: availableDecimal,
            currency: sourceAccount?.currencyCode ?? '',
          });
        }
      }
    }
  }

  const displayErrors = { ...clientErrors, ...backendErrors };
  const hasErrors = Object.keys(displayErrors).length > 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(amount);
    if (isNaN(parsed)) {
      window.alert(t('validation.invalidAmount'));
      return;
    }
    if (isBucket && hasErrors) {
      return;
    }
    setSubmitting(true);
    try {
      if (isBucket) {
        const bucketId = accountId;
        const apiErrors: Record<string, string> = {};
        for (const row of allocations) {
          if (row.sourceAccountId === null) continue;
          const sourceAccount = realAccounts.find((a) => a.accountId === row.sourceAccountId);
          const sourceMinorUnits = sourceAccount?.currencyMinorUnits ?? 2;
          let amountMinor: number;
          if (row.isUnlinked) {
            amountMinor = 0;
          } else {
            const parsedAmount = parseFloat(row.amountStr);
            if (isNaN(parsedAmount) || parsedAmount < 0) {
              apiErrors[row.tempId] = t('validation.invalidAmount');
              continue;
            }
            amountMinor = Math.round(parsedAmount * Math.pow(10, sourceMinorUnits));
            if (!row.isNew && amountMinor === row.originalAmountMinor) {
              continue; // unchanged existing allocation, skip
            }
          }
          try {
            await createBucketAllocation(bucketId, row.sourceAccountId, amountMinor, date);
          } catch (err) {
            const errStr = String(err);
            if (errStr.includes('OVER_ALLOCATION')) {
              apiErrors[row.tempId] = t('errors.overAllocation');
            } else {
              throw err;
            }
          }
        }
        if (Object.keys(apiErrors).length > 0) {
          setBackendErrors(apiErrors);
          setSubmitting(false);
          return;
        }
      }
      const amountMinor = Math.round(parsed * Math.pow(10, minorUnits));
      onSubmit(accountId, amountMinor, date, note);
    } catch (err) {
      window.alert(String(err));
      setSubmitting(false);
    }
  };

  const visibleAllocations = allocations.filter((row) => !row.isUnlinked);
  const linkedAccountIds = new Set(
    visibleAllocations
      .filter((r) => r.sourceAccountId !== null)
      .map((r) => r.sourceAccountId as number),
  );
  const availableToLink = realAccounts.filter((a) => !linkedAccountIds.has(a.accountId));

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.createBalanceUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label>{t('modals.createBalanceUpdate.account')}</Label>
            <Select value={String(accountId)} onValueChange={handleAccountChange}>
              <SelectTrigger>
                <SelectValue placeholder={t('modals.createBalanceUpdate.selectAccount')} />
              </SelectTrigger>
              <SelectContent>
                {accounts.map((a) => (
                  <SelectItem key={a.accountId} value={String(a.accountId)}>
                    {a.accountName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="cbu-amount">
              {isBucket
                ? t('modals.createBalanceUpdate.extraBalance')
                : t('modals.createBalanceUpdate.amount')}
            </Label>
            <div className="relative">
              <Input
                id="cbu-amount"
                type="number"
                step={minorUnits === 0 ? '1' : '0.' + '0'.repeat(minorUnits - 1) + '1'}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                className={currencyCode ? 'pr-14' : undefined}
                required
              />
              {currencyCode && (
                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground pointer-events-none">
                  {currencyCode}
                </span>
              )}
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="cbu-date">{t('modals.createBalanceUpdate.date')}</Label>
            <DatePicker id="cbu-date" value={date} onChange={setDate} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="cbu-note">{t('modals.createBalanceUpdate.note')}</Label>
            <Input
              id="cbu-note"
              type="text"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder={t('modals.createBalanceUpdate.notePlaceholder')}
            />
          </div>

          {isBucket && (
            <>
              <hr className="border-border" />
              <div className="flex flex-col gap-3">
                <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  {t('modals.createBalanceUpdate.linkedAccounts')}
                </p>

                {loadingAllocations ? (
                  <p className="text-sm text-muted-foreground">…</p>
                ) : (
                  <>
                    {visibleAllocations.map((row) => {
                      const sourceAccount =
                        row.sourceAccountId !== null
                          ? realAccounts.find((a) => a.accountId === row.sourceAccountId)
                          : undefined;
                      const sourceMinorUnits = sourceAccount?.currencyMinorUnits ?? 2;
                      const sourceCurrencyCode = sourceAccount?.currencyCode ?? '';
                      const available = getAvailableBalance(row);
                      const displayError = displayErrors[row.tempId];
                      const showAmountRow = !row.isNew || row.sourceAccountId !== null;

                      return (
                        <div
                          key={row.tempId}
                          className="flex flex-col gap-2 rounded-md border border-border bg-muted/30 p-3"
                        >
                          {row.isNew ? (
                            <Select
                              value={
                                row.sourceAccountId !== null ? String(row.sourceAccountId) : ''
                              }
                              onValueChange={(val) =>
                                void handleSourceAccountSelect(row.tempId, Number(val))
                              }
                            >
                              <SelectTrigger>
                                <SelectValue
                                  placeholder={t('modals.createBalanceUpdate.selectSourceAccount')}
                                />
                              </SelectTrigger>
                              <SelectContent>
                                {availableToLink.map((a) => (
                                  <SelectItem key={a.accountId} value={String(a.accountId)}>
                                    {a.accountName}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                          ) : (
                            <p className="text-sm font-medium">
                              {sourceAccount?.accountName ?? ''}
                            </p>
                          )}

                          {showAmountRow && (
                            <div className="flex items-center gap-2">
                              <div className="relative flex-1">
                                <Input
                                  type="number"
                                  step={
                                    sourceMinorUnits === 0
                                      ? '1'
                                      : '0.' + '0'.repeat(sourceMinorUnits - 1) + '1'
                                  }
                                  value={row.amountStr}
                                  onChange={(e) =>
                                    handleAllocationAmountChange(row.tempId, e.target.value)
                                  }
                                  className={sourceCurrencyCode ? 'pr-14' : undefined}
                                  placeholder={t('modals.createBalanceUpdate.allocationAmount')}
                                />
                                {sourceCurrencyCode && (
                                  <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground pointer-events-none">
                                    {sourceCurrencyCode}
                                  </span>
                                )}
                              </div>
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() =>
                                  row.isNew ? handleRemoveNew(row.tempId) : handleUnlink(row.tempId)
                                }
                              >
                                {t('modals.createBalanceUpdate.unlinkAccount')}
                              </Button>
                            </div>
                          )}

                          {showAmountRow && (
                            <p className="text-xs text-muted-foreground">
                              {t('modals.createBalanceUpdate.availableBalance', {
                                amount: (available / Math.pow(10, sourceMinorUnits)).toFixed(
                                  sourceMinorUnits,
                                ),
                                currency: sourceCurrencyCode,
                              })}
                            </p>
                          )}

                          {row.isNew && row.sourceAccountId === null && (
                            <div className="flex justify-end">
                              <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                onClick={() => handleRemoveNew(row.tempId)}
                              >
                                {t('modals.createBalanceUpdate.unlinkAccount')}
                              </Button>
                            </div>
                          )}

                          {displayError && (
                            <p className="text-xs text-destructive">{displayError}</p>
                          )}
                        </div>
                      );
                    })}

                    {availableToLink.length > 0 ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={handleAddAllocation}
                        className="self-start"
                      >
                        {t('modals.createBalanceUpdate.linkAccount')}
                      </Button>
                    ) : (
                      realAccounts.length === 0 && (
                        <p className="text-sm text-muted-foreground">
                          {t('modals.createBalanceUpdate.noAccountsToLink')}
                        </p>
                      )
                    )}
                  </>
                )}
              </div>
            </>
          )}

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createBalanceUpdate.cancel')}
            </Button>
            <Button type="submit" disabled={submitting || hasErrors}>
              {t('modals.createBalanceUpdate.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
