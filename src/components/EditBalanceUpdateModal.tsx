import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EventWithData } from '../types';
import { formatDate } from '../utils/format';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { DatePicker } from './ui/date-picker';

interface Props {
  event: EventWithData;
  onSubmit: (eventId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

export default function EditBalanceUpdateModal({ event, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [amount, setAmount] = useState((event.amountMinor / 100).toFixed(2));
  const [date, setDate] = useState(formatDate(event.eventDate));
  const [note, setNote] = useState(event.note ?? '');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(amount);
    if (isNaN(parsed)) {
      window.alert(t('validation.invalidAmount'));
      return;
    }
    const amountMinor = Math.round(parsed * 100);
    onSubmit(event.id, amountMinor, date, note);
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
          <DialogTitle>{t('modals.editBalanceUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label>{t('modals.editBalanceUpdate.account')}</Label>
            <Input type="text" value={event.accountName} disabled className="bg-muted" />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ebu-amount">{t('modals.editBalanceUpdate.amount')}</Label>
            <Input
              id="ebu-amount"
              type="number"
              step="0.01"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              required
              autoFocus
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ebu-date">{t('modals.editBalanceUpdate.date')}</Label>
            <DatePicker id="ebu-date" value={date} onChange={setDate} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ebu-note">{t('modals.editBalanceUpdate.note')}</Label>
            <Input
              id="ebu-note"
              type="text"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder={t('modals.editBalanceUpdate.notePlaceholder')}
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editBalanceUpdate.cancel')}
            </Button>
            <Button type="submit">{t('modals.editBalanceUpdate.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
