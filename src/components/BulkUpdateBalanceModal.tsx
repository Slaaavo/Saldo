import React, { useState } from 'react';
import type { SnapshotRow } from '../types';
import NumberValue from './NumberValue';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';

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
  const [date, setDate] = useState(selectedDate);
  const [note, setNote] = useState('');
  const [amounts, setAmounts] = useState<Record<number, string>>({});
  const [submitting, setSubmitting] = useState(false);

  const realAccounts = accounts.filter((r) => r.accountType === 'account');
  const bucketAccounts = accounts.filter((r) => r.accountType === 'bucket');

  const renderRow = (row: SnapshotRow) => (
    <React.Fragment key={row.accountId}>
      <div>
        <div className="text-sm font-medium">{row.accountName}</div>
        <NumberValue value={row.balanceMinor} className="text-xs text-muted-foreground" />
      </div>
      <Input
        type="number"
        step="0.01"
        placeholder="€"
        value={amounts[row.accountId] ?? ''}
        onChange={(e) => handleAmountChange(row.accountId, e.target.value)}
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
          updates.push({ accountId: account.accountId, amountMinor: Math.round(parsed * 100) });
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
      window.alert(`Failed to update balances: ${err}`);
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
          <DialogTitle>Update All Balances</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="bulk-date">Date</Label>
            <Input
              id="bulk-date"
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              required
            />
          </div>

          <hr className="border-border" />

          <div className="grid grid-cols-[auto_1fr] items-center gap-x-8 gap-y-3">
            {realAccounts.length > 0 && (
              <div className="col-span-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground pt-2">
                Accounts
              </div>
            )}
            {realAccounts.map(renderRow)}

            {bucketAccounts.length > 0 && (
              <div className="col-span-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground pt-4">
                Buckets
              </div>
            )}
            {bucketAccounts.map(renderRow)}
          </div>

          <hr className="border-border" />

          <div className="flex flex-col gap-2">
            <Label htmlFor="bulk-note">Note</Label>
            <Input
              id="bulk-note"
              type="text"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Note (optional)"
            />
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={submitting}>
              Update
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
