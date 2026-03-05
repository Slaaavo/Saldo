import { useState, useCallback, useEffect } from 'react';
import type { EventWithData, SnapshotRow } from './types';
import {
  getAccountsSnapshot,
  listEvents,
  createBalanceUpdate,
  createAccount,
  updateAccount,
  deleteAccount,
  updateEvent,
  deleteEvent,
  bulkCreateBalanceUpdates,
} from './api';
import { toEndOfDay, todayIso } from './utils/format';
import { cn } from '@/lib/utils';
import Header from './components/Header';
import NumberValue from './components/NumberValue';
import AccountCards from './components/AccountCards';
import Ledger from './components/Ledger';
import CreateBalanceUpdateModal from './components/CreateBalanceUpdateModal';
import EditBalanceUpdateModal from './components/EditBalanceUpdateModal';
import CreateAccountModal from './components/CreateAccountModal';
import RenameAccountModal from './components/RenameAccountModal';
import ConfirmDialog from './components/ConfirmDialog';
import BulkUpdateBalanceModal from './components/BulkUpdateBalanceModal';

type ModalState =
  | { type: 'none' }
  | { type: 'createBalanceUpdate'; preselectedAccountId?: number }
  | { type: 'editBalanceUpdate'; event: EventWithData }
  | { type: 'createAccount'; accountType?: 'account' | 'bucket' }
  | { type: 'renameAccount'; accountId: number; currentName: string }
  | {
      type: 'confirmDeleteAccount';
      accountId: number;
      name: string;
      accountType?: 'account' | 'bucket';
    }
  | { type: 'confirmDeleteEvent'; eventId: number }
  | { type: 'bulkUpdateBalance' };

function App() {
  const [selectedDate, setSelectedDate] = useState(todayIso());
  const [snapshot, setSnapshot] = useState<SnapshotRow[]>([]);
  const [events, setEvents] = useState<EventWithData[]>([]);
  const [modalState, setModalState] = useState<ModalState>({ type: 'none' });

  const closeModal = () => setModalState({ type: 'none' });

  const refresh = useCallback(async () => {
    try {
      const endOfDay = toEndOfDay(selectedDate);
      const [snap, evts] = await Promise.all([
        getAccountsSnapshot(endOfDay),
        listEvents(undefined, endOfDay),
      ]);
      setSnapshot(snap);
      setEvents(evts);
    } catch (err) {
      window.alert(`Failed to load data: ${err}`);
    }
  }, [selectedDate]);

  useEffect(() => {
    const load = async () => {
      await refresh();
    };
    load();
  }, [refresh]);

  const handleCreateBalanceUpdate = async (
    accountId: number,
    amountMinor: number,
    eventDate: string,
    note: string,
  ) => {
    try {
      await createBalanceUpdate(accountId, amountMinor, eventDate, note || undefined);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(`Failed to create balance update: ${err}`);
    }
  };

  const handleEditBalanceUpdate = async (
    eventId: number,
    amountMinor: number,
    eventDate: string,
    note: string,
  ) => {
    try {
      await updateEvent(eventId, amountMinor, eventDate, note || undefined);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(`Failed to update event: ${err}`);
    }
  };

  const handleDeleteEvent = async (eventId: number) => {
    try {
      await deleteEvent(eventId);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(`Failed to delete event: ${err}`);
    }
  };

  const handleCreateAccount = async (
    name: string,
    initialBalanceMinor?: number,
    accountType?: string,
  ) => {
    try {
      // TODO: hardcoded EUR currency ID = 1 for MVP; support multiple currencies later
      await createAccount(name, 1, initialBalanceMinor, accountType);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(`Failed to create account: ${err}`);
    }
  };

  const handleRenameAccount = async (accountId: number, name: string) => {
    try {
      await updateAccount(accountId, name);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(`Failed to rename account: ${err}`);
    }
  };

  const handleDeleteAccount = async (accountId: number) => {
    try {
      await deleteAccount(accountId);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(`Failed to delete account: ${err}`);
    }
  };

  const handleBulkUpdateSubmit = async (
    updates: { accountId: number; amountMinor: number }[],
    eventDate: string,
    note: string,
  ) => {
    await bulkCreateBalanceUpdates(updates, eventDate, note || undefined);
    closeModal();
    await refresh();
  };

  const accounts = snapshot.filter((r) => r.accountType === 'account');
  const buckets = snapshot.filter((r) => r.accountType === 'bucket');
  const totalMinor = accounts.reduce((sum, r) => sum + r.balanceMinor, 0);
  const bucketsMinor = buckets.reduce((sum, r) => sum + r.balanceMinor, 0);
  const leftToSpendMinor = totalMinor - bucketsMinor;

  return (
    <div className="min-h-screen bg-background">
      <div className="mx-auto max-w-4xl px-4 md:px-6 py-6">
        <div className="bg-card rounded-xl shadow-sm overflow-hidden">
          <Header selectedDate={selectedDate} onDateChange={setSelectedDate} />

          {/* Hero metrics */}
          <div
            className={cn(
              'px-4 md:px-10 py-10',
              buckets.length > 0 ? 'grid grid-cols-2' : 'flex justify-center',
            )}
          >
            <div className="flex flex-col items-center">
              <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">
                Total Balance
              </p>
              <p className={cn('text-5xl font-extrabold', totalMinor < 0 && 'text-destructive')}>
                <NumberValue value={totalMinor} />
              </p>
            </div>
            {buckets.length > 0 && (
              <div className="flex flex-col items-center">
                <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">
                  Left to Spend
                </p>
                <p
                  className={cn(
                    'text-5xl font-extrabold',
                    leftToSpendMinor < 0 && 'text-destructive',
                  )}
                >
                  <NumberValue value={leftToSpendMinor} />
                </p>
              </div>
            )}
          </div>

          <hr className="border-border" />

          {/* Accounts */}
          <div className="px-4 md:px-10 py-8">
            <AccountCards
              snapshot={accounts}
              sectionTitle="Accounts"
              addButtonLabel="Add Account"
              onUpdateBalance={(accountId) =>
                setModalState({ type: 'createBalanceUpdate', preselectedAccountId: accountId })
              }
              onRenameAccount={(accountId, currentName) =>
                setModalState({ type: 'renameAccount', accountId, currentName })
              }
              onDeleteAccount={(accountId, name) =>
                setModalState({
                  type: 'confirmDeleteAccount',
                  accountId,
                  name,
                  accountType: 'account',
                })
              }
              onCreateAccount={() =>
                setModalState({ type: 'createAccount', accountType: 'account' })
              }
            />
          </div>

          <hr className="border-border" />

          {/* Buckets */}
          <div className="px-4 md:px-10 py-8">
            <AccountCards
              snapshot={buckets}
              sectionTitle="Buckets"
              addButtonLabel="Add Bucket"
              emptyMessage="No buckets yet. Create one to allocate your capital."
              onUpdateBalance={(accountId) =>
                setModalState({ type: 'createBalanceUpdate', preselectedAccountId: accountId })
              }
              onRenameAccount={(accountId, currentName) =>
                setModalState({ type: 'renameAccount', accountId, currentName })
              }
              onDeleteAccount={(accountId, name) =>
                setModalState({
                  type: 'confirmDeleteAccount',
                  accountId,
                  name,
                  accountType: 'bucket',
                })
              }
              onCreateAccount={() =>
                setModalState({ type: 'createAccount', accountType: 'bucket' })
              }
            />
          </div>

          {/* Ledger */}
          <div className="px-4 md:px-10 py-8 border-t border-border">
            <Ledger
              events={events}
              accounts={snapshot}
              onEditEvent={(event) => setModalState({ type: 'editBalanceUpdate', event })}
              onDeleteEvent={(eventId) => setModalState({ type: 'confirmDeleteEvent', eventId })}
              onUpdateBalances={() => setModalState({ type: 'bulkUpdateBalance' })}
            />
          </div>
        </div>
      </div>

      {modalState.type === 'createBalanceUpdate' && (
        <CreateBalanceUpdateModal
          accounts={snapshot}
          preselectedAccountId={modalState.preselectedAccountId}
          onSubmit={handleCreateBalanceUpdate}
          onClose={closeModal}
        />
      )}

      {modalState.type === 'editBalanceUpdate' && (
        <EditBalanceUpdateModal
          event={modalState.event}
          onSubmit={handleEditBalanceUpdate}
          onClose={closeModal}
        />
      )}

      {modalState.type === 'createAccount' && (
        <CreateAccountModal
          accountType={modalState.accountType}
          onSubmit={handleCreateAccount}
          onClose={closeModal}
        />
      )}

      {modalState.type === 'renameAccount' && (
        <RenameAccountModal
          accountId={modalState.accountId}
          currentName={modalState.currentName}
          onSubmit={handleRenameAccount}
          onClose={closeModal}
        />
      )}

      {modalState.type === 'confirmDeleteAccount' && (
        <ConfirmDialog
          message={`Are you sure you want to delete ${modalState.accountType === 'bucket' ? 'bucket' : 'account'} "${modalState.name}"? This will also delete all its events.`}
          onConfirm={() => handleDeleteAccount(modalState.accountId)}
          onCancel={closeModal}
        />
      )}

      {modalState.type === 'confirmDeleteEvent' && (
        <ConfirmDialog
          message="Are you sure you want to delete this event?"
          onConfirm={() => handleDeleteEvent(modalState.eventId)}
          onCancel={closeModal}
        />
      )}

      {modalState.type === 'bulkUpdateBalance' && (
        <BulkUpdateBalanceModal
          accounts={snapshot}
          selectedDate={selectedDate}
          onSubmit={handleBulkUpdateSubmit}
          onClose={closeModal}
        />
      )}
    </div>
  );
}

export default App;
