import { useState } from 'react';
import type { SnapshotRow } from '../types';
import { todayIso } from '../utils/format';

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
  const [accountId, setAccountId] = useState<number>(
    preselectedAccountId ?? accounts[0]?.accountId ?? 0,
  );
  const [amount, setAmount] = useState('');
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = parseFloat(amount);
    if (isNaN(parsed)) {
      window.alert('Please enter a valid amount.');
      return;
    }
    const amountMinor = Math.round(parsed * 100);
    onSubmit(accountId, amountMinor, date, note);
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <h3>Create Balance Update</h3>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Account</label>
            <select value={accountId} onChange={(e) => setAccountId(Number(e.target.value))}>
              {accounts.map((a) => (
                <option key={a.accountId} value={a.accountId}>
                  {a.accountName}
                </option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label>Amount (€)</label>
            <input
              type="number"
              step="0.01"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              required
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
              Create
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
