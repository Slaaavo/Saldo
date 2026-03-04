import { useState } from 'react';
import { useEscapeKey } from '../hooks/useEscapeKey';

interface Props {
  accountId: number;
  currentName: string;
  onSubmit: (accountId: number, name: string) => void;
  onClose: () => void;
}

export default function RenameAccountModal({ accountId, currentName, onSubmit, onClose }: Props) {
  const [name, setName] = useState(currentName);

  useEscapeKey(onClose);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      window.alert('Account name is required.');
      return;
    }
    onSubmit(accountId, name.trim());
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title-rename-account"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 id="dialog-title-rename-account">Rename Account</h3>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>New Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
              autoFocus
            />
          </div>
          <div className="modal-actions">
            <button type="submit" className="btn btn-primary">
              Rename
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
