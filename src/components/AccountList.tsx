import type { SnapshotRow } from '../types';
import { formatEur } from '../utils/format';

interface Props {
  snapshot: SnapshotRow[];
  onUpdateBalance: (accountId: number) => void;
  onRenameAccount: (accountId: number, currentName: string) => void;
  onDeleteAccount: (accountId: number, name: string) => void;
  onCreateAccount: () => void;
}

export default function AccountList({
  snapshot,
  onUpdateBalance,
  onRenameAccount,
  onDeleteAccount,
  onCreateAccount,
}: Props) {
  return (
    <section className="account-list">
      <div className="section-header">
        <h2>Accounts</h2>
        <button className="btn btn-primary" onClick={onCreateAccount}>
          Add Account
        </button>
      </div>

      {snapshot.length === 0 ? (
        <p className="empty-message">No accounts yet. Create one to get started.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Account</th>
              <th className="text-right">Balance</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {snapshot.map((row) => (
              <tr key={row.accountId}>
                <td>{row.accountName}</td>
                <td className={`text-right ${row.balanceMinor < 0 ? 'negative' : ''}`}>
                  {formatEur(row.balanceMinor)}
                </td>
                <td className="actions">
                  <button className="btn btn-sm" onClick={() => onUpdateBalance(row.accountId)}>
                    Update Balance
                  </button>
                  <button
                    className="btn btn-sm"
                    onClick={() => onRenameAccount(row.accountId, row.accountName)}
                  >
                    Rename
                  </button>
                  <button
                    className="btn btn-sm btn-danger"
                    onClick={() => onDeleteAccount(row.accountId, row.accountName)}
                  >
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
