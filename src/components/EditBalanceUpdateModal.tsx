import { useState } from 'react';
import type { EventWithData } from '../types';
import { formatDate } from '../utils/format';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';

interface Props {
  event: EventWithData;
  onSubmit: (eventId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

export default function EditBalanceUpdateModal({ event, onSubmit, onClose }: Props) {
  const [amount, setAmount] = useState((event.amountMinor / 100).toFixed(2));
  const [date, setDate] = useState(formatDate(event.eventDate));
  const [note, setNote] = useState(event.note ?? '');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(amount);
    if (isNaN(parsed)) {
      window.alert('Please enter a valid amount.');
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
          <DialogTitle>Edit Balance Update</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label>Account</Label>
            <Input type="text" value={event.accountName} disabled className="bg-muted" />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ebu-amount">Amount (€)</Label>
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
            <Label htmlFor="ebu-date">Date</Label>
            <Input
              id="ebu-date"
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              required
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ebu-note">Note</Label>
            <Input
              id="ebu-note"
              type="text"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Optional note"
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit">Save</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
