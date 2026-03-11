import { useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import Header from './components/Header';
import AppModals from './components/AppModals';
import SettingsPage from './pages/SettingsPage';
import FxRatesPage from './pages/FxRatesPage';
import UnitsPage from './pages/UnitsPage';
import Sidebar from './components/Sidebar';
import { useTheme } from './hooks/useTheme';
import { useModalManager } from './hooks/useModalManager';
import { useFinanceData } from './hooks/useFinanceData';
import { useDbLocation } from './hooks/useDbLocation';
import { useDemoMode } from './hooks/useDemoMode';
import DashboardView from './pages/DashboardView';
import DemoModeBanner from './components/DemoModeBanner';
import { Toaster } from 'sonner';
import { computeDashboardMetrics } from './utils/dashboardMetrics';

const PAGE_TITLES: Record<string, string> = {
  dashboard: 'sidebar.dashboard',
  'fx-rates': 'sidebar.fxRates',
  units: 'sidebar.units',
  settings: 'sidebar.settings',
};

function App() {
  const { t } = useTranslation();
  const { theme, themePreference, setThemePreference } = useTheme();
  const { modalState, setModalState, closeModal } = useModalManager();
  const [currentView, setCurrentView] = useState<'dashboard' | 'fx-rates' | 'units' | 'settings'>(
    'dashboard',
  );
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const {
    selectedDate,
    setSelectedDate,
    snapshot,
    events,
    consolidationCurrency,
    refresh,
    handleConsolidationCurrencyChange,
    missingFxCurrencies,
  } = useFinanceData();

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

                  {currentView === 'dashboard' && (
                    <DashboardView
                      snapshot={snapshot}
                      accounts={accounts}
                      buckets={buckets}
                      assets={assets}
                      events={events}
                      consolidationCurrency={consolidationCurrency}
                      totalMinor={liquidMinor}
                      leftToSpendMinor={leftToSpendMinor}
                      netWorthMinor={netWorthMinor}
                      hasAssets={hasAssets}
                      missingFxCurrencies={missingFxCurrencies}
                      setModalState={setModalState}
                      isDemoMode={demo.isDemoMode}
                      onEnterDemoMode={demo.handleEnter}
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
            refresh={refresh}
            dbLocation={dbLocation}
          />
        </div>
      </div>
    </>
  );
}

export default App;
