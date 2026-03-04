import { useState } from 'react';
import type { EventWithData } from '../types';
import { formatDate } from '../utils/format';
import { useEscapeKey } from '../hooks/useEscapeKey';

interface Props {
  event: EventWithData;
  onSubmit: (eventId: number, amountMinor: number, eventDate: string, note: string) => void;
  onClose: () => void;
}

export default function EditBalanceUpdateModal({ event, onSubmit, onClose }: Props) {
  const [amount, setAmount] = useState((event.amountMinor / 100).toFixed(2));
  const [date, setDate] = useState(formatDate(event.eventDate));
  const [note, setNote] = useState(event.note ?? '');

  useEscapeKey(onClose);

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
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title-edit-balance-update"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 id="dialog-title-edit-balance-update">Edit Balance Update</h3>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Account</label>
            <input type="text" value={event.accountName} disabled />
          </div>
          <div className="form-group">
            <label>Amount (€)</label>
            <input
              type="number"
              step="0.01"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              required
              autoFocus
            />
          </div>
          <div className="form-group">
            <label>Date</label>
            <input type="date" value={date} onChange={(e) => setDate(e.target.value)} required />
          </div>
          <div className="form-group">
            <label>Note</label>
            <input
              type="text"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Optional note"
            />
          </div>
          <div className="modal-actions">
            <button type="submit" className="btn btn-primary">
              Save
            </button>
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
