import { useState } from 'react';

interface Props {
  onSubmit: (name: string, initialBalanceMinor?: number) => void;
  onClose: () => void;
}

export default function CreateAccountModal({ onSubmit, onClose }: Props) {
  const [name, setName] = useState('');
  const [balance, setBalance] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      window.alert('Account name is required.');
      return;
    }
    let initialBalanceMinor: number | undefined;
    if (balance.trim()) {
      const parsed = parseFloat(balance);
      if (isNaN(parsed)) {
        window.alert('Please enter a valid balance amount.');
        return;
      }
      initialBalanceMinor = Math.round(parsed * 100);
    }
    onSubmit(name.trim(), initialBalanceMinor);
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <h3>Create Account</h3>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Account Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Checking Account"
              required
            />
          </div>
          <div className="form-group">
            <label>Initial Balance (€, optional)</label>
            <input
              type="number"
              step="0.01"
              value={balance}
              onChange={(e) => setBalance(e.target.value)}
              placeholder="0.00"
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
