import { Fragment } from 'react';
import { useTranslation } from 'react-i18next';
import type { ImportRow, IbanMatchResult } from '../types';
import type { SnapshotRow } from '../../../../shared/types';
import { cn } from '../../../../shared/lib/utils';
import { Button } from '../../../../shared/ui/button';
import { Checkbox } from '../../../../shared/ui/checkbox';
import { DialogFooter } from '../../../../shared/ui/dialog';
import NumberValue from '../../../../shared/ui/NumberValue';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../../../shared/ui/select';
import { PartnerCell } from './PartnerCell';
import { SplitEditorPanel } from './SplitEditorPanel';

export interface ReviewStepProps {
  importRows: ImportRow[];
  selectedCount: number;
  duplicateCount: number;
  nearDateSkippedCount: number;
  balanceWarningDates: string[];
  availableBuckets: SnapshotRow[];
  accountsWithoutIban: SnapshotRow[];
  allAccounts: SnapshotRow[];
  selectedAccountCurrencyCode: string;
  selectedAccountMinorUnits: number;
  importing: boolean;
  onToggleRow: (index: number) => void;
  onSelectAll: () => void;
  onDeselectAll: () => void;
  onBucketChange: (index: number, bucketId: number | null) => void;
  onCreatePartner: (iban: string, name: string) => Promise<void>;
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>;
  onCounterpartChange: (index: number, accountId: number | null) => void;
  onImport: () => Promise<void>;
  onBack: () => void;
  onCancel: () => void;
  onSplitOpen: (rowIndex: number) => void;
  onSplitCancel: (rowIndex: number) => void;
  onLegAmountChange: (rowIndex: number, legIndex: number, amountMinor: number) => void;
  onLegNoteChange: (rowIndex: number, legIndex: number, note: string | null) => void;
  onLegPartnerChange: (
    rowIndex: number,
    legIndex: number,
    rawIban: string | null,
    ibanMatch: IbanMatchResult,
    counterpartAccountId: number | null,
  ) => void;
  onLegBucketChange: (rowIndex: number, legIndex: number, bucketId: number | null) => void;
  onAddLeg: (rowIndex: number) => void;
  onRemoveLeg: (rowIndex: number, legIndex: number) => void;
  splitValidationErrors: string[];
}

const BUCKET_NONE = '__none__';

function ReviewTableColGroup() {
  return (
    <colgroup>
      <col className="w-8" />
      <col className="w-24" />
      <col className="w-40" />
      <col />
      <col className="w-32" />
      <col className="w-40" />
      <col className="w-20" />
    </colgroup>
  );
}

export default function ReviewStep({
  importRows,
  selectedCount,
  duplicateCount,
  nearDateSkippedCount,
  balanceWarningDates,
  availableBuckets,
  accountsWithoutIban,
  allAccounts,
  selectedAccountCurrencyCode,
  selectedAccountMinorUnits,
  importing,
  onToggleRow,
  onSelectAll,
  onDeselectAll,
  onBucketChange,
  onCreatePartner,
  onAssignIban,
  onCounterpartChange,
  onImport,
  onBack,
  onCancel,
  onSplitOpen,
  onSplitCancel,
  onLegAmountChange,
  onLegNoteChange,
  onLegPartnerChange,
  onLegBucketChange,
  onAddLeg,
  onRemoveLeg,
  splitValidationErrors,
}: ReviewStepProps) {
  const { t } = useTranslation();

  const total = importRows.length;
  const allSelected = selectedCount === total && total > 0;
  const someSelected = selectedCount > 0 && selectedCount < total;

  const selectAllState: boolean | 'indeterminate' = allSelected
    ? true
    : someSelected
      ? 'indeterminate'
      : false;

  const handleSelectAll = () => {
    if (allSelected || someSelected) {
      onDeselectAll();
    } else {
      onSelectAll();
    }
  };

  // Track first occurrence of each unmatched IBAN
  const seenIbans = new Set<string>();
  const firstOccurrenceSet = new Set<number>();
  for (const row of importRows) {
    if (row.ibanMatch.type === 'unmatched' && row.rawIban) {
      const key = row.rawIban;
      if (!seenIbans.has(key)) {
        seenIbans.add(key);
        firstOccurrenceSet.add(row.index);
      }
    }
  }

  return (
    <>
      <div className="flex flex-col gap-3 min-h-0">
        <p className="text-sm text-muted-foreground">
          {t('import.reviewStep.summary', { total, duplicates: duplicateCount })}
        </p>

        {balanceWarningDates.length > 0 && (
          <div className="rounded-md border border-blue-200 bg-blue-50 dark:border-blue-800 dark:bg-blue-950 p-3 text-sm text-blue-800 dark:text-blue-200">
            {t('import.reviewStep.balanceWarning', {
              dates: balanceWarningDates.join(', '),
            })}
          </div>
        )}

        <div className="flex items-center gap-2">
          <Checkbox checked={selectAllState} onCheckedChange={handleSelectAll} id="select-all" />
          <label htmlFor="select-all" className="text-sm cursor-pointer select-none">
            {selectedCount > 0
              ? t('import.reviewStep.selectAll') + ` (${selectedCount}/${total})`
              : t('import.reviewStep.selectAll')}
          </label>
        </div>

        <div className="border border-border rounded-[var(--radius)] bg-background overflow-hidden">
          {/* Static header — outside the scroll context, no sticky needed */}
          <table className="w-full text-sm table-fixed border-separate border-spacing-0">
            <ReviewTableColGroup />
            <thead>
              <tr>
                <th className="px-2 py-2 border-b border-border" />
                <th className="px-2 py-2 text-left font-medium text-muted-foreground whitespace-nowrap border-b border-border">
                  {t('import.mappingStep.field.date')}
                </th>
                <th className="px-2 py-2 text-right font-medium text-muted-foreground whitespace-nowrap border-b border-border">
                  {t('import.mappingStep.field.amount')}
                </th>
                <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">
                  {t('import.mappingStep.field.partner')}
                </th>
                <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">
                  {t('import.mappingStep.field.note')}
                </th>
                <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">
                  {t('import.reviewStep.bucket')}
                </th>
                <th className="px-2 py-2 text-left font-medium text-muted-foreground border-b border-border">
                  {t('import.reviewStep.split.actionsColumn')}
                </th>
              </tr>
            </thead>
          </table>
          {/* Scrollable body only — scrollbar-gutter:stable keeps width aligned with header */}
          <div className="max-h-[360px] overflow-y-auto [scrollbar-gutter:stable]">
            <table className="w-full text-sm table-fixed border-separate border-spacing-0">
              <ReviewTableColGroup />
              <tbody>
                {importRows.map((row) => {
                  const isFirstOccurrence = firstOccurrenceSet.has(row.index);
                  const isSplit = row.splitLegs !== null;
                  return (
                    <Fragment key={row.index}>
                      <tr
                        className={cn(
                          'group align-top',
                          (row.isDuplicate || row.nearDateDuplicateEventId !== null) &&
                            'bg-amber-50 dark:bg-amber-950/20',
                          !row.isSelected && 'opacity-50',
                          isSplit && 'bg-muted/30',
                        )}
                      >
                        <td className="px-2 py-2 border-b border-border group-last:border-b-0">
                          <Checkbox
                            checked={row.isSelected}
                            onCheckedChange={() => onToggleRow(row.index)}
                          />
                        </td>
                        <td className="px-2 py-2 whitespace-nowrap border-b border-border group-last:border-b-0">
                          <div className="flex flex-col gap-0.5">
                            <span>{row.date.substring(0, 10)}</span>
                            {row.isDuplicate && (
                              <span className="text-xs font-medium text-amber-600 dark:text-amber-400">
                                {t('import.reviewStep.duplicate')}
                              </span>
                            )}
                            {row.nearDateDuplicateEventId !== null && (
                              <span className="text-xs font-medium text-amber-600 dark:text-amber-400">
                                {t('import.reviewStep.duplicateNearDate')}
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="px-2 py-2 text-right whitespace-nowrap border-b border-border group-last:border-b-0">
                          <NumberValue
                            value={row.amountMinor}
                            minorUnits={selectedAccountMinorUnits}
                            currencyCode={selectedAccountCurrencyCode}
                          />
                        </td>
                        <td className="px-2 py-2 border-b border-border group-last:border-b-0">
                          {isSplit ? (
                            <span className="text-muted-foreground">—</span>
                          ) : (
                            <PartnerCell
                              row={row}
                              accountsWithoutIban={accountsWithoutIban}
                              allAccounts={allAccounts}
                              onCreatePartner={onCreatePartner}
                              onAssignIban={onAssignIban}
                              onCounterpartChange={onCounterpartChange}
                              isFirstOccurrence={isFirstOccurrence}
                            />
                          )}
                        </td>
                        <td className="px-2 py-2 border-b border-border group-last:border-b-0">
                          <span
                            className="text-muted-foreground truncate block"
                            title={row.note ?? ''}
                          >
                            {row.note ?? '—'}
                          </span>
                        </td>
                        <td className="px-2 py-2 border-b border-border group-last:border-b-0">
                          {isSplit ? (
                            <span className="text-muted-foreground">—</span>
                          ) : (
                            <Select
                              value={row.bucketId !== null ? String(row.bucketId) : BUCKET_NONE}
                              onValueChange={(v) =>
                                onBucketChange(row.index, v === BUCKET_NONE ? null : Number(v))
                              }
                            >
                              <SelectTrigger className="h-7 text-xs">
                                <SelectValue placeholder={t('import.reviewStep.selectBucket')} />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value={BUCKET_NONE}>
                                  {t('import.reviewStep.selectBucket')}
                                </SelectItem>
                                {availableBuckets.map((b) => (
                                  <SelectItem key={b.accountId} value={String(b.accountId)}>
                                    {b.accountName}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                          )}
                        </td>
                        <td className="px-2 py-2 border-b border-border group-last:border-b-0">
                          {isSplit ? (
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              className="h-7 text-xs px-2"
                              onClick={() => onSplitCancel(row.index)}
                            >
                              {t('import.reviewStep.split.cancelButton')}
                            </Button>
                          ) : (
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              className="h-7 text-xs px-2"
                              onClick={() => onSplitOpen(row.index)}
                            >
                              {t('import.reviewStep.split.splitButton')}
                            </Button>
                          )}
                        </td>
                      </tr>
                      {isSplit && (
                        <tr key={`split-editor-${row.index}`}>
                          <td colSpan={7} className="p-0">
                            <SplitEditorPanel
                              row={row}
                              selectedAccountCurrencyCode={selectedAccountCurrencyCode}
                              selectedAccountMinorUnits={selectedAccountMinorUnits}
                              availableBuckets={availableBuckets}
                              accountsWithoutIban={accountsWithoutIban}
                              allAccounts={allAccounts}
                              onLegAmountChange={onLegAmountChange}
                              onLegNoteChange={onLegNoteChange}
                              onLegPartnerChange={onLegPartnerChange}
                              onLegBucketChange={onLegBucketChange}
                              onAddLeg={onAddLeg}
                              onRemoveLeg={onRemoveLeg}
                              onCreatePartner={onCreatePartner}
                              onAssignIban={onAssignIban}
                            />
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      {splitValidationErrors.length > 0 && (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          <p className="font-medium">{t('import.reviewStep.split.validationBanner')}</p>
          {splitValidationErrors.map((err, i) => (
            <p key={i} className="text-xs mt-1">
              {err}
            </p>
          ))}
        </div>
      )}

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack} disabled={importing}>
          {t('import.back')}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel} disabled={importing}>
          {t('modals.confirm.cancel')}
        </Button>
        <div className="flex flex-col items-end gap-1">
          <Button
            type="button"
            onClick={onImport}
            disabled={
              (selectedCount === 0 && nearDateSkippedCount === 0) ||
              importing ||
              splitValidationErrors.length > 0
            }
          >
            {importing ? '...' : t('import.reviewStep.importButton', { count: selectedCount })}
          </Button>
          {nearDateSkippedCount > 0 && (
            <p className="text-xs text-muted-foreground">
              {t('import.reviewStep.updateTransfersNote', { count: nearDateSkippedCount })}
            </p>
          )}
        </div>
      </DialogFooter>
    </>
  );
}
