import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { fetchFxRates, enterDemoMode, exitDemoMode, isDemoMode as checkIsDemoMode } from './api';
import Header from './components/Header';
import CreateBalanceUpdateModal from './components/CreateBalanceUpdateModal';
import EditBalanceUpdateModal from './components/EditBalanceUpdateModal';
import CreateAccountModal from './components/CreateAccountModal';
import RenameAccountModal from './components/RenameAccountModal';
import ConfirmDialog from './components/ConfirmDialog';
import BulkUpdateBalanceModal from './components/BulkUpdateBalanceModal';
import SettingsPage from './pages/SettingsPage';
import FxRatesPage from './pages/FxRatesPage';
import ReorderModal from './components/ReorderModal';
import Sidebar from './components/Sidebar';
import { useTheme } from './hooks/useTheme';
import { useModalManager } from './hooks/useModalManager';
import { useFinanceData } from './hooks/useFinanceData';
import DashboardView from './pages/DashboardView';
import DemoModeBanner from './components/DemoModeBanner';
import { Toaster, toast } from 'sonner';

function App() {
  const { t } = useTranslation();
  const { theme, themePreference, setThemePreference } = useTheme();
  const { modalState, setModalState, closeModal } = useModalManager();
  const [currentView, setCurrentView] = useState<'dashboard' | 'fx-rates' | 'settings'>(
    'dashboard',
  );
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [fetchLoading, setFetchLoading] = useState(false);
  const [isDemoMode, setIsDemoMode] = useState<boolean>(false);

  useEffect(() => {
    checkIsDemoMode().then(setIsDemoMode).catch(console.error);
  }, []);

  const handleEnterDemoMode = async () => {
    try {
      await enterDemoMode();
      setIsDemoMode(true);
      await refresh();
      setCurrentView('dashboard');
    } catch (err) {
      toast.error(String(err));
    }
  };

  const handleExitDemoMode = async () => {
    try {
      await exitDemoMode();
      setIsDemoMode(false);
      await refresh();
      toast.success(t('demo.exitedToast'));
    } catch (err) {
      toast.error(String(err));
    }
  };

  const {
    selectedDate,
    setSelectedDate,
    snapshot,
    events,
    consolidationCurrency,
    refresh,
    handleCreateBalanceUpdate,
    handleEditBalanceUpdate,
    handleDeleteEvent,
    handleCreateAccount,
    handleRenameAccount,
    handleDeleteAccount,
    handleBulkUpdateSubmit,
    handleSaveOrder,
    handleConsolidationCurrencyChange,
    missingFxCurrencies,
  } = useFinanceData({
    closeModal,
    onFxRatePrompt: (date) => setModalState({ type: 'fetchFxRatePrompt', date }),
  });

  const accounts = snapshot.filter((r) => r.accountType === 'account');
  const buckets = snapshot.filter((r) => r.accountType === 'bucket');
  const totalMinor = accounts.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const bucketsMinor = buckets.reduce((sum, r) => sum + r.convertedBalanceMinor, 0);
  const leftToSpendMinor = totalMinor - bucketsMinor;

  const pageTitle =
    currentView === 'settings'
      ? t('sidebar.settings')
      : currentView === 'fx-rates'
        ? t('sidebar.fxRates')
        : t('sidebar.dashboard');

  return (
    <>
      <Toaster theme={theme} richColors />
      <div className="flex flex-col h-screen">
        {isDemoMode && <DemoModeBanner onExit={handleExitDemoMode} />}
        <div className="flex flex-1 min-h-0 bg-background overflow-hidden">
          <Sidebar
            currentView={currentView}
            onNavigate={setCurrentView}
            collapsed={sidebarCollapsed}
            onToggleCollapse={() => setSidebarCollapsed((c) => !c)}
          />
          <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
            <div className="flex-1 overflow-y-auto [scrollbar-gutter:stable]">
              <div className="mx-auto max-w-4xl py-6 px-4">
                <div className="bg-card rounded-xl shadow-sm overflow-hidden">
                  <Header
                    pageTitle={pageTitle}
                    selectedDate={selectedDate}
                    onDateChange={setSelectedDate}
                    showDatePicker={currentView === 'dashboard'}
                  />

                  {currentView === 'fx-rates' && <FxRatesPage />}

                  {currentView === 'settings' && (
                    <SettingsPage
                      onConsolidationCurrencyChange={handleConsolidationCurrencyChange}
                      themePreference={themePreference}
                      onThemeChange={setThemePreference}
                      isDemoMode={isDemoMode}
                      onEnterDemoMode={handleEnterDemoMode}
                      onExitDemoMode={handleExitDemoMode}
                    />
                  )}

                  {currentView === 'dashboard' && (
                    <DashboardView
                      snapshot={snapshot}
                      accounts={accounts}
                      buckets={buckets}
                      events={events}
                      consolidationCurrency={consolidationCurrency}
                      totalMinor={totalMinor}
                      leftToSpendMinor={leftToSpendMinor}
                      missingFxCurrencies={missingFxCurrencies}
                      setModalState={setModalState}
                      isDemoMode={isDemoMode}
                      onEnterDemoMode={handleEnterDemoMode}
                    />
                  )}
                </div>
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
                entityType: t(
                  modalState.accountType === 'bucket' ? 'common.bucket' : 'common.account',
                ),
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

          {modalState.type === 'reorderAccounts' && (
            <ReorderModal
              items={accounts.map((r) => ({ id: r.accountId, name: r.accountName }))}
              title={t('reorder.titleAccounts')}
              onSave={handleSaveOrder}
              onClose={closeModal}
            />
          )}

          {modalState.type === 'reorderBuckets' && (
            <ReorderModal
              items={buckets.map((r) => ({ id: r.accountId, name: r.accountName }))}
              title={t('reorder.titleBuckets')}
              onSave={handleSaveOrder}
              onClose={closeModal}
            />
          )}
        </div>
      </div>
    </>
  );
}

export default App;
