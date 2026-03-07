import { useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { EventWithData, SnapshotRow, Currency } from './types';
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
  getConsolidationCurrency,
  fetchFxRates,
  listFxRates,
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
import SettingsPanel from './components/SettingsPanel';
import FxRatesPage from './components/FxRatesPage';

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
  | { type: 'bulkUpdateBalance' }
  | { type: 'fetchFxRatePrompt'; date: string };

function App() {
  const { t } = useTranslation();
  const tRef = useRef(t);
  useEffect(() => {
    tRef.current = t;
  }, [t]);
  const [selectedDate, setSelectedDate] = useState(todayIso());
  const [snapshot, setSnapshot] = useState<SnapshotRow[]>([]);
  const [events, setEvents] = useState<EventWithData[]>([]);
  const [modalState, setModalState] = useState<ModalState>({ type: 'none' });
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null);
  const [currentView, setCurrentView] = useState<'dashboard' | 'fx-rates'>('dashboard');
  const [showSettings, setShowSettings] = useState(false);
  const [showFxRateBanner, setShowFxRateBanner] = useState(false);
  const [fetchLoading, setFetchLoading] = useState(false);

  useEffect(() => {
    getConsolidationCurrency()
      .then(setConsolidationCurrency)
      .catch((err) => console.error('Failed to load consolidation currency:', err));
  }, []);

  useEffect(() => {
    fetchFxRates().catch(() => setShowFxRateBanner(true));
  }, []);

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
      window.alert(tRef.current('errors.loadData', { error: String(err) }));
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
    const account = snapshot.find((r) => r.accountId === accountId);
    try {
      await createBalanceUpdate(accountId, amountMinor, eventDate, note || undefined);
      closeModal();
      await refresh();
      // Step 3.4: prompt to fetch FX rates when saving a non-consolidation-currency balance update
      if (account && consolidationCurrency && account.currencyCode !== consolidationCurrency.code) {
        const rates = await listFxRates(eventDate);
        const hasRate = rates.some((r) => r.toCurrencyCode === account.currencyCode);
        if (!hasRate) {
          setModalState({ type: 'fetchFxRatePrompt', date: eventDate });
        }
      }
    } catch (err) {
      window.alert(t('errors.createBalanceUpdate', { error: String(err) }));
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
      window.alert(t('errors.updateEvent', { error: String(err) }));
    }
  };

  const handleDeleteEvent = async (eventId: number) => {
    try {
      await deleteEvent(eventId);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(t('errors.deleteEvent', { error: String(err) }));
    }
  };

  const handleCreateAccount = async (
    name: string,
    currencyId: number,
    initialBalanceMinor?: number,
    accountType?: string,
  ) => {
    try {
      await createAccount(name, currencyId, initialBalanceMinor, accountType);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(t('errors.createAccount', { error: String(err) }));
    }
  };

  const handleRenameAccount = async (accountId: number, name: string) => {
    try {
      await updateAccount(accountId, name);
      closeModal();
      await refresh();
    } catch (err) {
      window.alert(t('errors.renameAccount', { error: String(err) }));
    }
  };

  const handleDeleteAccount = async (accountId: number) => {
    try {
      await deleteAccount(accountId);
      closeModal();
      await refresh();
    } catch (err) {
      const msg = String(err);
      if (msg.includes('active allocations in buckets')) {
        window.alert(t('errors.deleteAccountLinked'));
      } else {
        window.alert(t('errors.deleteAccount', { error: msg }));
      }
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

  const handleConsolidationCurrencyChange = useCallback(async () => {
    try {
      const currency = await getConsolidationCurrency();
      setConsolidationCurrency(currency);
      await refresh();
    } catch (err) {
      console.error('Failed to reload after currency change:', err);
    }
  }, [refresh]);

  const accounts = snapshot.filter((r) => r.accountType === 'account');
  const buckets = snapshot.filter((r) => r.accountType === 'bucket');
  const totalMinor = accounts.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const bucketsMinor = buckets.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const leftToSpendMinor = totalMinor - bucketsMinor;
  const missingFxCurrencies = [
    ...new Set(snapshot.filter((r) => r.fxRateMissing).map((r) => r.currencyCode)),
  ];

  return (
    <div className="min-h-screen bg-background">
      <div className="mx-auto max-w-4xl px-4 md:px-6 py-6">
        <div className="bg-card rounded-xl shadow-sm overflow-hidden">
          <Header
            selectedDate={selectedDate}
            onDateChange={setSelectedDate}
            currentView={currentView}
            onNavigate={setCurrentView}
            onOpenSettings={() => setShowSettings(true)}
          />

          {/* Offline FX rate banner */}
          {showFxRateBanner && (
            <div className="mx-4 md:mx-10 mt-4 flex items-center justify-between rounded-md border border-orange-400 bg-orange-50 px-4 py-2 text-sm text-orange-800">
              <span>{t('banner.fxRatesUnavailable')}</span>
              <button
                onClick={() => setShowFxRateBanner(false)}
                className="ml-4 font-medium hover:opacity-70"
                aria-label={t('banner.dismiss')}
              >
                ✕
              </button>
            </div>
          )}

          {currentView === 'fx-rates' ? (
            <FxRatesPage />
          ) : (
            <>
              {/* FX rate missing warning */}
              {missingFxCurrencies.length > 0 && (
                <div className="mx-4 md:mx-10 mb-4 rounded-md border border-yellow-400 bg-yellow-50 px-4 py-2 text-sm text-yellow-800">
                  {t('metrics.fxRateMissing', { currencies: missingFxCurrencies.join(', ') })}
                </div>
              )}

              {/* Hero metrics */}
              <div
                className={cn(
                  'px-4 md:px-10 py-10',
                  buckets.length > 0 ? 'grid grid-cols-2' : 'flex justify-center',
                )}
              >
                <div className="flex flex-col items-center">
                  <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">
                    {t('metrics.totalBalance')}
                  </p>
                  <p
                    className={cn('text-5xl font-extrabold', totalMinor < 0 && 'text-destructive')}
                  >
                    <NumberValue
                      value={totalMinor}
                      currencyCode={consolidationCurrency?.code}
                      minorUnits={consolidationCurrency?.minorUnits ?? 2}
                    />
                  </p>
                </div>
                {buckets.length > 0 && (
                  <div className="flex flex-col items-center">
                    <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">
                      {t('metrics.leftToSpend')}
                    </p>
                    <p
                      className={cn(
                        'text-5xl font-extrabold',
                        leftToSpendMinor < 0 && 'text-destructive',
                      )}
                    >
                      <NumberValue
                        value={leftToSpendMinor}
                        currencyCode={consolidationCurrency?.code}
                        minorUnits={consolidationCurrency?.minorUnits ?? 2}
                      />
                    </p>
                  </div>
                )}
              </div>

              <hr className="border-border" />

              {/* Accounts */}
              <div className="px-4 md:px-10 py-8">
                <AccountCards
                  snapshot={accounts}
                  consolidationCurrency={consolidationCurrency}
                  sectionTitle={t('accounts.sectionTitle')}
                  addButtonLabel={t('accounts.addAccount')}
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
                  consolidationCurrency={consolidationCurrency}
                  sectionTitle={t('buckets.sectionTitle')}
                  addButtonLabel={t('buckets.addBucket')}
                  emptyMessage={t('buckets.empty')}
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
                  consolidationCurrency={consolidationCurrency}
                  onEditEvent={(event) => setModalState({ type: 'editBalanceUpdate', event })}
                  onDeleteEvent={(eventId) =>
                    setModalState({ type: 'confirmDeleteEvent', eventId })
                  }
                  onUpdateBalances={() => setModalState({ type: 'bulkUpdateBalance' })}
                />
              </div>
            </>
          )}
        </div>
      </div>

      {showSettings && (
        <SettingsPanel
          onClose={() => setShowSettings(false)}
          onConsolidationCurrencyChange={handleConsolidationCurrencyChange}
        />
      )}

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
          accounts={snapshot}
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
          message={t('modals.confirm.deleteAccount', {
            entityType: t(modalState.accountType === 'bucket' ? 'common.bucket' : 'common.account'),
            name: modalState.name,
          })}
          onConfirm={() => handleDeleteAccount(modalState.accountId)}
          onCancel={closeModal}
        />
      )}

      {modalState.type === 'confirmDeleteEvent' && (
        <ConfirmDialog
          message={t('modals.confirm.deleteEvent')}
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

      {modalState.type === 'fetchFxRatePrompt' && (
        <ConfirmDialog
          message={t('modals.fetchFxRatePrompt.message', { date: modalState.date })}
          confirmVariant="default"
          loading={fetchLoading}
          onConfirm={async () => {
            setFetchLoading(true);
            try {
              await fetchFxRates(modalState.date);
              await refresh();
            } catch {
              // Silently ignore fetch errors — user can retry from FX Rates screen
            } finally {
              setFetchLoading(false);
              closeModal();
            }
          }}
          onCancel={closeModal}
        />
      )}
    </div>
  );
}

export default App;
