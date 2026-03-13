import { useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import Header from '../shared/layout/Header';
import AppModals from './AppModals';
import SettingsPage from '../features/settings/SettingsPage';
import FxRatesPage from '../features/currency/FxRatesPage';
import UnitsPage from '../features/assets/UnitsPage';
import Sidebar from '../shared/layout/Sidebar';
import { useTheme } from '../features/settings/useTheme';
import { useModalManager } from './useModalManager';
import { useFinanceData } from '../features/dashboard/useFinanceData';
import { useDbLocation } from '../features/settings/useDbLocation';
import { useDemoMode } from '../features/settings/useDemoMode';
import DashboardView from '../features/dashboard/DashboardView';
import DemoModeBanner from '../features/settings/DemoModeBanner';
import LedgerPage from '../features/ledger/LedgerPage';
import { Toaster } from 'sonner';
import { computeDashboardMetrics } from '../features/dashboard/dashboardMetrics';

const PAGE_TITLES: Record<string, string> = {
  dashboard: 'sidebar.dashboard',
  ledger: 'sidebar.ledger',
  'fx-rates': 'sidebar.fxRates',
  units: 'sidebar.units',
  settings: 'sidebar.settings',
};

function App() {
  const { t } = useTranslation();
  const { theme, themePreference, setThemePreference } = useTheme();
  const { modalState, setModalState, closeModal } = useModalManager();
  const [currentView, setCurrentView] = useState<
    'dashboard' | 'ledger' | 'fx-rates' | 'units' | 'settings'
  >('dashboard');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [ledgerRefreshCounter, setLedgerRefreshCounter] = useState(0);

  const {
    selectedDate,
    setSelectedDate,
    snapshot,
    events,
    totalEvents,
    consolidationCurrency,
    refresh,
    handleConsolidationCurrencyChange,
    missingFxCurrencies,
  } = useFinanceData();

  const refreshAll = useCallback(async () => {
    await refresh();
    setLedgerRefreshCounter((c) => c + 1);
  }, [refresh]);

  const dbLocation = useDbLocation({
    setModalState,
    closeModal,
    onAfterDbChange: handleConsolidationCurrencyChange,
  });

  const demo = useDemoMode({
    refresh,
    loadDbLocation: dbLocation.load,
    onEntered: () => setCurrentView('dashboard'),
  });

  const { accounts, buckets, assets, hasAssets, liquidMinor, leftToSpendMinor, netWorthMinor } =
    useMemo(() => computeDashboardMetrics(snapshot), [snapshot]);

  const pageTitle = t(PAGE_TITLES[currentView]);

  return (
    <>
      <Toaster theme={theme} richColors />
      <div className="flex flex-col h-screen">
        {demo.isDemoMode && <DemoModeBanner onExit={demo.handleExit} />}
        <div className="flex flex-1 min-h-0 bg-background overflow-hidden">
          <Sidebar
            currentView={currentView}
            onNavigate={setCurrentView}
            collapsed={sidebarCollapsed}
            onToggleCollapse={() => setSidebarCollapsed((c) => !c)}
          />
          <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
            <div className="flex-1 overflow-y-auto [scrollbar-gutter:stable]">
              <div className="mx-auto max-w-5xl py-6 px-4">
                <div className="bg-card rounded-xl shadow-sm overflow-hidden">
                  <Header
                    pageTitle={pageTitle}
                    selectedDate={selectedDate}
                    onDateChange={setSelectedDate}
                    showDatePicker={currentView === 'dashboard'}
                  />

                  {currentView === 'fx-rates' && <FxRatesPage />}

                  {currentView === 'units' && <UnitsPage />}

                  {currentView === 'settings' && (
                    <SettingsPage
                      onConsolidationCurrencyChange={handleConsolidationCurrencyChange}
                      themePreference={themePreference}
                      onThemeChange={setThemePreference}
                      isDemoMode={demo.isDemoMode}
                      onEnterDemoMode={demo.handleEnter}
                      onExitDemoMode={demo.handleExit}
                      dbLocationPath={dbLocation.path}
                      dbLocationIsDefault={dbLocation.isDefault}
                      onChangeDbLocation={dbLocation.handleChange}
                      onResetDbLocation={dbLocation.handleReset}
                    />
                  )}

                  {currentView === 'ledger' && (
                    <LedgerPage
                      snapshot={snapshot}
                      consolidationCurrency={consolidationCurrency}
                      setModalState={setModalState}
                      refreshTrigger={ledgerRefreshCounter}
                    />
                  )}

                  {currentView === 'dashboard' && (
                    <DashboardView
                      snapshot={snapshot}
                      accounts={accounts}
                      buckets={buckets}
                      assets={assets}
                      events={events}
                      totalEvents={totalEvents}
                      consolidationCurrency={consolidationCurrency}
                      totalMinor={liquidMinor}
                      leftToSpendMinor={leftToSpendMinor}
                      netWorthMinor={netWorthMinor}
                      hasAssets={hasAssets}
                      missingFxCurrencies={missingFxCurrencies}
                      setModalState={setModalState}
                      isDemoMode={demo.isDemoMode}
                      onEnterDemoMode={demo.handleEnter}
                      onNavigate={(v) =>
                        setCurrentView(
                          v as 'dashboard' | 'ledger' | 'fx-rates' | 'units' | 'settings',
                        )
                      }
                    />
                  )}
                </div>
              </div>
            </div>
          </div>

          <AppModals
            modalState={modalState}
            closeModal={closeModal}
            setModalState={setModalState}
            snapshot={snapshot}
            accounts={accounts}
            buckets={buckets}
            assets={assets}
            selectedDate={selectedDate}
            consolidationCurrency={consolidationCurrency}
            refresh={refreshAll}
            dbLocation={dbLocation}
          />
        </div>
      </div>
    </>
  );
}

export default App;
