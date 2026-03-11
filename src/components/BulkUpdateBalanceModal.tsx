import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { ChevronDown, ChevronRight, Link2 } from 'lucide-react';
import { toMinorUnits, getMinorUnitsStep } from '../utils/format';
import { extractErrorMessage } from '../utils/errors';
import type { SnapshotRow } from '../types';
import NumberValue from './NumberValue';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from './ui/dialog';
import { Button } from './ui/button';
import { CurrencyInput } from './CurrencyInput';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { DatePicker } from './ui/date-picker';

interface Props {
  accounts: SnapshotRow[];
  selectedDate: string;
  onSubmit: (
    updates: { accountId: number; amountMinor: number }[],
    eventDate: string,
    note: string,
  ) => Promise<void>;
  onClose: () => void;
}

export default function BulkUpdateBalanceModal({
  accounts,
  selectedDate,
  onSubmit,
  onClose,
}: Props) {
  const { t } = useTranslation();
  const [date, setDate] = useState(selectedDate);
  const [note, setNote] = useState('');
  const [amounts, setAmounts] = useState<Record<number, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [accountsExpanded, setAccountsExpanded] = useState(true);
  const [bucketsExpanded, setBucketsExpanded] = useState(true);
  const [assetsExpanded, setAssetsExpanded] = useState(true);

  const realAccounts = accounts.filter((r) => r.accountType === 'account');
  const bucketAccounts = accounts.filter((r) => r.accountType === 'bucket');
  const assetAccounts = accounts.filter((r) => r.accountType === 'asset');

  const renderRow = (row: SnapshotRow) => (
    <React.Fragment key={row.accountId}>
      <div className="flex-col flex">
        <div className="flex items-center gap-1.5 text-sm font-medium">
          {row.isLinkedToAsset && <Link2 className="h-3.5 w-3.5 text-muted-foreground shrink-0" />}
          {row.accountName}
        </div>
        {row.accountType === 'bucket' && (
          <div className="text-xs text-muted-foreground">
            {t('modals.createBalanceUpdate.extraBalance')}
          </div>
        )}
        <NumberValue
          value={row.balanceMinor}
          currencyCode={row.currencyCode}
          minorUnits={row.currencyMinorUnits}
          className="text-xs text-muted-foreground"
        />
      </div>
      <CurrencyInput
        type="number"
        step={getMinorUnitsStep(row.currencyMinorUnits)}
        value={amounts[row.accountId] ?? ''}
        onChange={(e) => handleAmountChange(row.accountId, e.target.value)}
        currencyCode={row.currencyCode}
      />
    </React.Fragment>
  );

  const handleAmountChange = (accountId: number, value: string) => {
    setAmounts((prev) => ({ ...prev, [accountId]: value }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);

    const updates: { accountId: number; amountMinor: number }[] = [];
    for (const account of accounts) {
      const raw = amounts[account.accountId];
      if (raw !== undefined && raw !== '') {
        const parsed = parseFloat(raw);
        if (isFinite(parsed)) {
          updates.push({
            accountId: account.accountId,
            amountMinor: toMinorUnits(raw, account.currencyMinorUnits),
          });
        }
      }
    }

    if (updates.length === 0) {
      setSubmitting(false);
      return;
    }

    try {
      await onSubmit(updates, date, note);
    } catch (err) {
      toast.error(t('errors.updateBalances', { error: extractErrorMessage(err) }));
      setSubmitting(false);
    }
  };

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.bulkUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="bulk-date">{t('modals.bulkUpdate.date')}</Label>
              <DatePicker id="bulk-date" value={date} onChange={setDate} />
            </div>

            <hr className="border-border" />

            <div className="grid grid-cols-[auto_1fr] items-center gap-x-8 gap-y-3">
              {realAccounts.length > 0 && (
                <button
                  type="button"
                  className="col-span-2 flex items-center gap-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground pt-2"
                  onClick={() => setAccountsExpanded((v) => !v)}
                >
                  {accountsExpanded ? (
                    <ChevronDown className="h-3.5 w-3.5" />
                  ) : (
                    <ChevronRight className="h-3.5 w-3.5" />
                  )}
                  {t('modals.bulkUpdate.accountsSection')}
                </button>
              )}
              {accountsExpanded && realAccounts.map(renderRow)}

              {bucketAccounts.length > 0 && (
                <button
                  type="button"
                  className="col-span-2 flex items-center gap-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground pt-4"
                  onClick={() => setBucketsExpanded((v) => !v)}
                >
                  {bucketsExpanded ? (
                    <ChevronDown className="h-3.5 w-3.5" />
                  ) : (
                    <ChevronRight className="h-3.5 w-3.5" />
                  )}
                  {t('modals.bulkUpdate.bucketsSection')}
                </button>
              )}
              {bucketsExpanded && bucketAccounts.map(renderRow)}

              {assetAccounts.length > 0 && (
                <button
                  type="button"
                  className="col-span-2 flex items-center gap-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground pt-4"
                  onClick={() => setAssetsExpanded((v) => !v)}
                >
                  {assetsExpanded ? (
                    <ChevronDown className="h-3.5 w-3.5" />
                  ) : (
                    <ChevronRight className="h-3.5 w-3.5" />
                  )}
                  {t('modals.bulkUpdate.assetsSection')}
                </button>
              )}
              {assetsExpanded && assetAccounts.map(renderRow)}
            </div>

            <hr className="border-border" />

            <div className="flex flex-col gap-2">
              <Label htmlFor="bulk-note">{t('modals.bulkUpdate.note')}</Label>
              <Input
                id="bulk-note"
                type="text"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t('modals.bulkUpdate.notePlaceholder')}
              />
            </div>
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.bulkUpdate.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.bulkUpdate.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
