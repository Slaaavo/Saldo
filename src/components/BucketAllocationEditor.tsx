import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../types';
import type { AllocationRow } from '../hooks/useBucketAllocations';
import { getMinorUnitsStep, fromMinorUnits } from '../utils/format';
import { Button } from './ui/button';
import { CurrencyInput } from './CurrencyInput';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';

interface Props {
  visibleAllocations: AllocationRow[];
  availableToLink: SnapshotRow[];
  realAccounts: SnapshotRow[];
  loadingAllocations: boolean;
  displayErrors: Record<string, string>;
  getAvailableBalance: (row: AllocationRow) => number;
  handleSourceAccountSelect: (tempId: string, newSourceAccountId: number) => Promise<void>;
  handleAllocationAmountChange: (tempId: string, value: string) => void;
  handleAddAllocation: () => void;
  handleRemoveNew: (tempId: string) => void;
  handleUnlink: (tempId: string) => void;
}

export default function BucketAllocationEditor({
  visibleAllocations,
  availableToLink,
  realAccounts,
  loadingAllocations,
  displayErrors,
  getAvailableBalance,
  handleSourceAccountSelect,
  handleAllocationAmountChange,
  handleAddAllocation,
  handleRemoveNew,
  handleUnlink,
}: Props) {
  const { t } = useTranslation();

  return (
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
                      value={row.sourceAccountId !== null ? String(row.sourceAccountId) : ''}
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
                    <p className="text-sm font-medium">{sourceAccount?.accountName ?? ''}</p>
                  )}

                  {showAmountRow && (
                    <div className="flex items-center gap-2">
                      <div className="flex-1">
                        <CurrencyInput
                          type="number"
                          step={getMinorUnitsStep(sourceMinorUnits)}
                          value={row.amountStr}
                          onChange={(e) => handleAllocationAmountChange(row.tempId, e.target.value)}
                          currencyCode={sourceCurrencyCode || undefined}
                          placeholder={t('modals.createBalanceUpdate.allocationAmount')}
                        />
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
                        amount: fromMinorUnits(available, sourceMinorUnits),
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

                  {displayError && <p className="text-xs text-destructive">{displayError}</p>}
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
  );
}
