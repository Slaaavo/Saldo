import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle } from 'lucide-react';
import type { ImportRow } from './types';
import type { SnapshotRow } from '../../../shared/types';
import { cn } from '../../../shared/lib/utils';
import { Button } from '../../../shared/ui/button';
import { Checkbox } from '../../../shared/ui/checkbox';
import { DialogFooter } from '../../../shared/ui/dialog';
import { Input } from '../../../shared/ui/input';
import NumberValue from '../../../shared/ui/NumberValue';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../../../shared/ui/select';

interface ReviewStepProps {
  importRows: ImportRow[];
  selectedCount: number;
  duplicateCount: number;
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
}

const BUCKET_NONE = '__none__';
const COUNTERPART_NONE = '__none__';

function IbanActionCell({
  row,
  accountsWithoutIban,
  onCreatePartner,
  onAssignIban,
  isFirstOccurrence,
}: {
  row: ImportRow;
  accountsWithoutIban: SnapshotRow[];
  onCreatePartner: (iban: string, name: string) => Promise<void>;
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>;
  isFirstOccurrence: boolean;
}) {
  const { t } = useTranslation();
  const [partnerName, setPartnerName] = useState('');
  const [assignAccountId, setAssignAccountId] = useState<string>('');
  const [creating, setCreating] = useState(false);
  const [assigning, setAssigning] = useState(false);

  if (row.ibanMatch.type !== 'unmatched' || !row.rawIban) return null;

  if (!isFirstOccurrence) {
    return <span className="text-xs text-muted-foreground italic">↑ resolve above</span>;
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
            setCreating(true);
            await onCreatePartner(row.rawIban!, partnerName.trim());
            setCreating(false);
            setPartnerName('');
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
              setAssigning(true);
              await onAssignIban(row.rawIban!, Number(assignAccountId));
              setAssigning(false);
              setAssignAccountId('');
            }}
          >
            {t('import.reviewStep.assignToAccount')}
          </Button>
        </div>
      )}
    </div>
  );
}

function PartnerCell({
  row,
  accountsWithoutIban,
  allAccounts,
  onCreatePartner,
  onAssignIban,
  onCounterpartChange,
  isFirstOccurrence,
}: {
  row: ImportRow;
  accountsWithoutIban: SnapshotRow[];
  allAccounts: SnapshotRow[];
  onCreatePartner: (iban: string, name: string) => Promise<void>;
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>;
  onCounterpartChange: (index: number, accountId: number | null) => void;
  isFirstOccurrence: boolean;
}) {
  const { t } = useTranslation();

  switch (row.ibanMatch.type) {
    case 'partner':
      return (
        <div className="flex flex-col">
          <span className="text-sm">{row.ibanMatch.accountName}</span>
          {row.rawIban && (
            <span className="text-xs text-muted-foreground truncate max-w-[140px]">
              {row.rawIban}
            </span>
          )}
        </div>
      );

    case 'ownAccount':
      return (
        <div className="flex flex-col">
          <div className="flex items-center gap-1">
            <span className="text-sm">{row.ibanMatch.accountName}</span>
            <span className="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300">
              {t('import.reviewStep.transfer')}
            </span>
          </div>
        </div>
      );

    case 'unmatched':
      return (
        <div className="flex flex-col">
          <div className="flex items-center gap-1">
            <AlertTriangle className="h-3.5 w-3.5 text-amber-500 shrink-0" />
            <span className="text-xs text-muted-foreground truncate max-w-[120px]">
              {row.rawIban}
            </span>
          </div>
          <IbanActionCell
            row={row}
            accountsWithoutIban={accountsWithoutIban}
            onCreatePartner={onCreatePartner}
            onAssignIban={onAssignIban}
            isFirstOccurrence={isFirstOccurrence}
          />
        </div>
      );

    case 'none':
    default:
      return (
        <Select
          value={
            row.counterpartAccountId !== null ? String(row.counterpartAccountId) : COUNTERPART_NONE
          }
          onValueChange={(v) =>
            onCounterpartChange(row.index, v === COUNTERPART_NONE ? null : Number(v))
          }
        >
          <SelectTrigger className="h-7 text-xs max-w-[160px]">
            <SelectValue placeholder={t('import.reviewStep.assignCounterpart')} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={COUNTERPART_NONE}>—</SelectItem>
            {allAccounts.map((a) => (
              <SelectItem key={a.accountId} value={String(a.accountId)}>
                {a.accountName}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      );
  }
}

export default function ReviewStep({
  importRows,
  selectedCount,
  duplicateCount,
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
            <colgroup>
              <col className="w-8" />
              <col className="w-24" />
              <col className="w-32" />
              <col />
              <col className="w-32" />
              <col className="w-40" />
            </colgroup>
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
              </tr>
            </thead>
          </table>
          {/* Scrollable body only — scrollbar-gutter:stable keeps width aligned with header */}
          <div className="max-h-[360px] overflow-y-auto [scrollbar-gutter:stable]">
            <table className="w-full text-sm table-fixed border-separate border-spacing-0">
              <colgroup>
                <col className="w-8" />
                <col className="w-24" />
                <col className="w-32" />
                <col />
                <col className="w-32" />
                <col className="w-40" />
              </colgroup>
              <tbody>
                {importRows.map((row) => {
                  const isFirstOccurrence = firstOccurrenceSet.has(row.index);
                  return (
                    <tr
                      key={row.index}
                      className={cn(
                        'group align-top',
                        row.isDuplicate && 'bg-amber-50 dark:bg-amber-950/20',
                        !row.isSelected && 'opacity-50',
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
                        <PartnerCell
                          row={row}
                          accountsWithoutIban={accountsWithoutIban}
                          allAccounts={allAccounts}
                          onCreatePartner={onCreatePartner}
                          onAssignIban={onAssignIban}
                          onCounterpartChange={onCounterpartChange}
                          isFirstOccurrence={isFirstOccurrence}
                        />
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
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onBack} disabled={importing}>
          {t('import.back')}
        </Button>
        <Button type="button" variant="outline" onClick={onCancel} disabled={importing}>
          {t('modals.confirm.cancel')}
        </Button>
        <Button type="button" onClick={onImport} disabled={selectedCount === 0 || importing}>
          {importing ? '...' : t('import.reviewStep.importButton', { count: selectedCount })}
        </Button>
      </DialogFooter>
    </>
  );
}
