import { useTranslation } from 'react-i18next';
import { Check } from 'lucide-react';
import type { ImportRow, IbanMatchResult } from '../types';
import type { SnapshotRow } from '../../../../shared/types';
import { cn } from '../../../../shared/lib/utils';
import { Button } from '../../../../shared/ui/button';
import NumberValue from '../../../../shared/ui/NumberValue';
import { SplitLegRow } from './SplitLegRow';

function SplitEditorColGroup() {
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

interface SplitEditorPanelProps {
  row: ImportRow;
  selectedAccountCurrencyCode: string;
  selectedAccountMinorUnits: number;
  availableBuckets: SnapshotRow[];
  accountsWithoutIban: SnapshotRow[];
  allAccounts: SnapshotRow[];
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
  onCreatePartner: (iban: string, name: string) => Promise<void>;
  onAssignIban: (iban: string, targetAccountId: number) => Promise<void>;
}

export function SplitEditorPanel({
  row,
  selectedAccountCurrencyCode,
  selectedAccountMinorUnits,
  availableBuckets,
  accountsWithoutIban,
  allAccounts,
  onLegAmountChange,
  onLegNoteChange,
  onLegPartnerChange,
  onLegBucketChange,
  onAddLeg,
  onRemoveLeg,
  onCreatePartner,
  onAssignIban,
}: SplitEditorPanelProps) {
  const { t } = useTranslation();

  const legs = row.splitLegs!;
  const allocatedMinor = legs.reduce((sum, l) => sum + l.amountMinor, 0);
  const diff = row.amountMinor - allocatedMinor;
  const isBalanced = diff === 0;

  return (
    <div className="bg-muted/20 border-t border-border">
      <table className="w-full text-sm table-fixed border-separate border-spacing-0">
        <SplitEditorColGroup />
        <tbody>
          <tr className="bg-muted/40">
            <td colSpan={7} className="px-2 py-1 text-xs font-medium text-muted-foreground">
              {t('import.reviewStep.split.title', { amount: '' })}
              <NumberValue
                value={row.amountMinor}
                minorUnits={selectedAccountMinorUnits}
                currencyCode={selectedAccountCurrencyCode}
                className="ml-0"
              />
            </td>
          </tr>
          {legs.map((leg, i) => (
            <SplitLegRow
              key={leg.legIndex}
              leg={leg}
              legNumber={i + 1}
              selectedAccountMinorUnits={selectedAccountMinorUnits}
              selectedAccountCurrencyCode={selectedAccountCurrencyCode}
              availableBuckets={availableBuckets}
              accountsWithoutIban={accountsWithoutIban}
              allAccounts={allAccounts}
              canRemove={legs.length > 1}
              onAmountChange={(legIndex, amountMinor) =>
                onLegAmountChange(row.index, legIndex, amountMinor)
              }
              onNoteChange={(legIndex, note) => onLegNoteChange(row.index, legIndex, note)}
              onPartnerChange={(legIndex, rawIban, ibanMatch, counterpartAccountId) =>
                onLegPartnerChange(row.index, legIndex, rawIban, ibanMatch, counterpartAccountId)
              }
              onBucketChange={(legIndex, bucketId) =>
                onLegBucketChange(row.index, legIndex, bucketId)
              }
              onRemove={(legIndex) => onRemoveLeg(row.index, legIndex)}
              onCreatePartner={onCreatePartner}
              onAssignIban={onAssignIban}
            />
          ))}
          <tr className="align-top">
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5 text-xs text-muted-foreground">
              {t('import.reviewStep.split.remaining')}
            </td>
            <td className="px-2 py-1.5">
              <div
                className={cn(
                  'flex items-center justify-end gap-1 text-xs',
                  isBalanced ? 'text-green-600 dark:text-green-400' : 'text-destructive',
                )}
              >
                {isBalanced && <Check className="h-3.5 w-3.5 shrink-0" />}
                <NumberValue
                  value={diff}
                  minorUnits={selectedAccountMinorUnits}
                  currencyCode={selectedAccountCurrencyCode}
                />
              </div>
            </td>
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5" />
            <td className="px-2 py-1.5">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 text-xs whitespace-nowrap px-2"
                onClick={() => onAddLeg(row.index)}
              >
                {t('import.reviewStep.split.addSplit')}
              </Button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  );
}
