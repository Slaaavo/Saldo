import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../../shared/types';
import type { AllocationRow } from './useBucketAllocations';
import { getMinorUnitsStep, fromMinorUnits } from '../../shared/utils/format';
import { Button } from '../../shared/ui/button';
import { CurrencyInput } from '../../shared/ui/CurrencyInput';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '../../shared/ui/select';

interface Props {
  visibleAllocations: AllocationRow[];
  availableToLink: SnapshotRow[];
  allocationSources: SnapshotRow[];
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
  allocationSources,
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

  const accountOptions = availableToLink.filter((a) => a.accountType === 'account');
  const assetOptions = availableToLink.filter((a) => a.accountType === 'asset');

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
                  ? allocationSources.find((a) => a.accountId === row.sourceAccountId)
                  : undefined;
              const sourceMinorUnits = sourceAccount?.currencyMinorUnits ?? 2;
              const sourceCurrencyCode = sourceAccount?.currencyCode ?? '';
              const available = getAvailableBalance(row);
              const displayError = displayErrors[row.tempId];
              const showAmountRow = !row.isNew || row.sourceAccountId !== null;

              // When a new row has an account selected, that account is excluded from
              // availableToLink (to prevent double-linking). Ensure it still appears in
              // the dropdown so Radix Select can display its label.
              let extraAccountForRow: (typeof allocationSources)[0] | undefined;
              let extraAssetForRow: (typeof allocationSources)[0] | undefined;
              if (row.isNew && row.sourceAccountId !== null) {
                const selected = allocationSources.find((a) => a.accountId === row.sourceAccountId);
                if (
                  selected &&
                  !accountOptions.some((a) => a.accountId === row.sourceAccountId) &&
                  !assetOptions.some((a) => a.accountId === row.sourceAccountId)
                ) {
                  if (selected.accountType === 'account') {
                    extraAccountForRow = selected;
                  } else {
                    extraAssetForRow = selected;
                  }
                }
              }

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
                        <SelectGroup>
                          <SelectLabel>{t('modals.allocation.accountsGroup')}</SelectLabel>
                          {accountOptions.map((a) => (
                            <SelectItem key={a.accountId} value={String(a.accountId)}>
                              {a.accountName}
                            </SelectItem>
                          ))}
                          {extraAccountForRow && (
                            <SelectItem
                              key={extraAccountForRow.accountId}
                              value={String(extraAccountForRow.accountId)}
                            >
                              {extraAccountForRow.accountName}
                            </SelectItem>
                          )}
                        </SelectGroup>
                        {(assetOptions.length > 0 || extraAssetForRow) && (
                          <SelectGroup>
                            <SelectLabel>{t('modals.allocation.assetsGroup')}</SelectLabel>
                            {assetOptions.map((a) => (
                              <SelectItem key={a.accountId} value={String(a.accountId)}>
                                {a.accountName}
                              </SelectItem>
                            ))}
                            {extraAssetForRow && (
                              <SelectItem
                                key={extraAssetForRow.accountId}
                                value={String(extraAssetForRow.accountId)}
                              >
                                {extraAssetForRow.accountName}
                              </SelectItem>
                            )}
                          </SelectGroup>
                        )}
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
              allocationSources.length === 0 && (
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
