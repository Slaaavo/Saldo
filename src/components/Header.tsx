import type { SnapshotRow } from '../types';
import { formatEur } from '../utils/format';

interface Props {
  selectedDate: string;
  onDateChange: (date: string) => void;
  snapshot: SnapshotRow[];
}

export default function Header({ selectedDate, onDateChange, snapshot }: Props) {
  const totalMinor = snapshot.reduce((sum, r) => sum + r.balanceMinor, 0);

  return (
    <header className="app-header">
      <h1 className="app-title">Our Finances</h1>
      <div className="header-controls">
        <label>
          Date:{' '}
          <input
            type="date"
            value={selectedDate}
            onChange={(e) => onDateChange(e.target.value)}
          />
        </label>
        <span className={`total-balance ${totalMinor < 0 ? 'negative' : ''}`}>
          Total: {formatEur(totalMinor)}
        </span>
      </div>
    </header>
  );
}
