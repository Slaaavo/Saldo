import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../types';
import { todayIso } from '../utils/format';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { DatePicker } from './ui/date-picker';

interface Props {
  accounts: SnapshotRow[];
  preselectedAccountId?: number;
  onSubmit: (accountId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

export default function CreateBalanceUpdateModal({
  accounts,
  preselectedAccountId,
  onSubmit,
  onClose,
}: Props) {
  const { t } = useTranslation();
  const [accountId, setAccountId] = useState<number>(
    preselectedAccountId ?? accounts[0]?.accountId ?? 0,
  );
  const [amount, setAmount] = useState('');
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState('');

  const selectedAccount = accounts.find((a) => a.accountId === accountId);
  const minorUnits = selectedAccount?.currencyMinorUnits ?? 2;
  const currencyCode = selectedAccount?.currencyCode;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(amount);
    if (isNaN(parsed)) {
      window.alert(t('validation.invalidAmount'));
      return;
    }
    const amountMinor = Math.round(parsed * Math.pow(10, minorUnits));
    onSubmit(accountId, amountMinor, date, note);
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
          <DialogTitle>{t('modals.createBalanceUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label>{t('modals.createBalanceUpdate.account')}</Label>
            <Select value={String(accountId)} onValueChange={(val) => setAccountId(Number(val))}>
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
            <Label htmlFor="cbu-amount">{t('modals.createBalanceUpdate.amount')}</Label>
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
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createBalanceUpdate.cancel')}
            </Button>
            <Button type="submit">{t('modals.createBalanceUpdate.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
