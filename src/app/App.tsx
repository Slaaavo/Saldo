import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router'
import Header from '../shared/layout/Header'
import AppModals from './AppModals'
import Sidebar from '../shared/layout/Sidebar'
import { useTheme } from '../features/settings/useTheme'
import { useDbLocation } from '../features/settings/useDbLocation'
import { useDemoMode } from '../features/settings/useDemoMode'
import DemoModeBanner from '../features/settings/DemoModeBanner'
import { Toaster } from 'sonner'
import { todayIso } from '../shared/utils/format'
import { ModalProvider, useModal } from './ModalContext'
import { DemoProvider } from './DemoContext'
import { SelectedDateProvider } from './SelectedDateContext'

const PAGE_TITLES: Record<string, string> = {
  dashboard: 'sidebar.dashboard',
  ledger: 'sidebar.ledger',
  'fx-rates': 'sidebar.fxRates',
  units: 'sidebar.units',
  settings: 'sidebar.settings',
  partners: 'sidebar.partners',
  'import-profiles': 'sidebar.importProfiles',
}

// AppShell renders inside ModalProvider so it can safely call useModal()
function AppShell() {
  const { t } = useTranslation()
  const { theme } = useTheme()
  const { setModalState, closeModal } = useModal()
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)

  const navigate = useNavigate()

  const dbLocation = useDbLocation({
    setModalState,
    closeModal,
    onAfterDbChange: async () => {},
  })

  const demo = useDemoMode({
    loadDbLocation: dbLocation.load,
    onEntered: () => navigate({ to: '/dashboard' }),
  })

  const routerState = useRouterState()
  const pageTitle = t(PAGE_TITLES[routerState.location.pathname.slice(1)] ?? 'sidebar.dashboard')

  return (
    <SelectedDateProvider>
      <DemoProvider isDemoMode={demo.isDemoMode} onEnterDemoMode={demo.handleEnter} onExitDemoMode={demo.handleExit}>
        <>
          <Toaster theme={theme} richColors />
          <div className="flex flex-col h-screen">
            {demo.isDemoMode && <DemoModeBanner onExit={demo.handleExit} />}
            <div className="flex flex-1 min-h-0 bg-background overflow-hidden">
              <Sidebar collapsed={sidebarCollapsed} onToggleCollapse={() => setSidebarCollapsed((c) => !c)} />
              <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
                <div className="flex-1 overflow-y-auto [scrollbar-gutter:stable]">
                  <div className="mx-auto max-w-5xl py-6 px-4">
                    <div className="bg-card rounded-xl shadow-sm overflow-hidden">
                      <Header pageTitle={pageTitle} />
                      <Outlet />
                    </div>
                  </div>
                </div>
              </div>
              <AppModals selectedDate={todayIso()} dbLocation={dbLocation} />
            </div>
          </div>
        </>
      </DemoProvider>
    </SelectedDateProvider>
  )
}

function App() {
  return (
    <ModalProvider>
      <AppShell />
    </ModalProvider>
  )
}

export default App
