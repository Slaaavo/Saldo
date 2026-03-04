import type { EventWithData, SnapshotRow } from '../types';
import { formatDate, formatEur, toEndOfDay } from '../utils/format';

interface Props {
  events: EventWithData[];
  accounts: SnapshotRow[];
  selectedDate: string;
  filterAccountId: number | null;
  onFilterChange: (accountId: number | null) => void;
  onEditEvent: (event: EventWithData) => void;
  onDeleteEvent: (eventId: number) => void;
}

export default function Ledger({
  events,
  accounts,
  selectedDate,
  filterAccountId,
  onFilterChange,
  onEditEvent,
  onDeleteEvent,
}: Props) {
  const endOfDay = toEndOfDay(selectedDate);
  const filtered = events.filter(
    (e) =>
      e.eventDate <= endOfDay &&
      (filterAccountId === null || e.accountId === filterAccountId),
  );

  return (
    <section className="ledger">
      <div className="section-header">
        <h2>Ledger</h2>
        <label>
          Filter:{' '}
          <select
            value={filterAccountId ?? ''}
            onChange={(e) => onFilterChange(e.target.value ? Number(e.target.value) : null)}
          >
            <option value="">All Accounts</option>
            {accounts.map((a) => (
              <option key={a.accountId} value={a.accountId}>
                {a.accountName}
              </option>
            ))}
          </select>
        </label>
      </div>

      {filtered.length === 0 ? (
        <p className="empty-message">No events to display.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Date</th>
              <th>Account</th>
              <th className="text-right">Amount</th>
              <th>Note</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((ev) => (
              <tr key={ev.id}>
                <td>{formatDate(ev.eventDate)}</td>
                <td>{ev.accountName}</td>
                <td className={`text-right ${ev.amountMinor < 0 ? 'negative' : ''}`}>
                  {formatEur(ev.amountMinor)}
                </td>
                <td>{ev.note ?? ''}</td>
                <td className="actions">
                  <button className="btn btn-sm" onClick={() => onEditEvent(ev)}>
                    Edit
                  </button>
                  <button className="btn btn-sm btn-danger" onClick={() => onDeleteEvent(ev.id)}>
                    Delete
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
