import { useState, useEffect } from 'react';
import type { SnapshotRow } from '../types';
import { formatEur } from '../utils/format';

interface Props {
  selectedDate: string;
  onDateChange: (date: string) => void;
  snapshot: SnapshotRow[];
}

export default function Header({ selectedDate, onDateChange, snapshot }: Props) {
  const [localDate, setLocalDate] = useState(selectedDate);
  const totalMinor = snapshot.reduce((sum, r) => sum + r.balanceMinor, 0);

  // Sync local state if parent changes selectedDate externally
  useEffect(() => {
    setLocalDate(selectedDate);
  }, [selectedDate]);

  const commitDate = () => {
    if (localDate && localDate !== selectedDate) {
      onDateChange(localDate);
    }
  };

  return (
    <header className="app-header">
      <h1 className="app-title">Our Finances</h1>
      <div className="header-controls">
        <label>
          Date:{' '}
          <input
            type="date"
            value={localDate}
            onChange={(e) => setLocalDate(e.target.value)}
            onBlur={commitDate}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commitDate();
            }}
          />
        </label>
        <span className={`total-balance ${totalMinor < 0 ? 'negative' : ''}`}>
          Total: {formatEur(totalMinor)}
        </span>
      </div>
    </header>
  );
}
