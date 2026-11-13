import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { LayoutDashboard, Settings, ArrowLeftRight, ChevronsLeft, ChevronsRight, Layers, BookOpen, Users, FileText, Calculator, Handshake } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/shared/lib/utils'
import { Link } from '@tanstack/react-router'
import type { LinkProps } from '@tanstack/react-router'
import SaldoLogo from './SaldoLogo'
import saldoLogotype from '@/assets/Saldo logotype transparent.svg'
import saldoLogotypeDark from '@/assets/Saldo logotype transparent dark.svg'

interface SidebarProps {
  collapsed: boolean
  onToggleCollapse: () => void
}

interface NavItem {
  to: LinkProps['to']
  icon: LucideIcon
  labelKey: string
}

const MONEY_ITEMS: NavItem[] = [
  { to: '/dashboard', icon: LayoutDashboard, labelKey: 'sidebar.dashboard' },
  { to: '/ledger', icon: BookOpen, labelKey: 'sidebar.ledger' },
  { to: '/tax-models', icon: Calculator, labelKey: 'sidebar.taxModels' },
]

const SETUP_ITEMS: NavItem[] = [
  { to: '/persons', icon: Users, labelKey: 'sidebar.persons' },
  { to: '/partners', icon: Handshake, labelKey: 'sidebar.partners' },
  { to: '/import-profiles', icon: FileText, labelKey: 'sidebar.importProfiles' },
  { to: '/fx-rates', icon: ArrowLeftRight, labelKey: 'sidebar.fxRates' },
  { to: '/units', icon: Layers, labelKey: 'sidebar.units' },
]

const SETTINGS_ITEM: NavItem = { to: '/settings', icon: Settings, labelKey: 'sidebar.settings' }

const navLinkBase = 'w-full flex items-center gap-3 px-2 py-2 rounded-lg text-sm font-medium transition-colors'

const Sidebar = ({ collapsed, onToggleCollapse }: SidebarProps) => {
  const { t } = useTranslation()
  const [hovered, setHovered] = useState(false)

  const expanded = !collapsed || (collapsed && hovered)

  const renderItem = ({ to, icon: Icon, labelKey }: NavItem) => (
    <Link
      key={to}
      to={to}
      activeProps={{ className: cn(navLinkBase, 'bg-accent text-accent-foreground') }}
      inactiveProps={{ className: cn(navLinkBase, 'text-muted-foreground hover:bg-muted hover:text-foreground') }}
      activeOptions={{ exact: true, includeSearch: false }}
    >
      <Icon className="h-5 w-5 shrink-0" />
      <span className={cn('overflow-hidden whitespace-nowrap transition-opacity duration-150', expanded ? 'opacity-100' : 'opacity-0 w-0')}>{t(labelKey)}</span>
    </Link>
  )

  return (
    <nav
      className={cn('group relative flex flex-col bg-card border-r border-border shrink-0 transition-[width] duration-150 ease-in-out', expanded ? 'w-56' : 'w-14')}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* App name row */}
      <div className="flex items-center justify-center px-4 py-3 h-14 shrink-0">
        {expanded ? (
          <>
            <img src={saldoLogotype} alt="Saldo" className="h-8 w-auto dark:hidden" />
            <img src={saldoLogotypeDark} alt="Saldo" className="hidden h-8 w-auto dark:block" />
          </>
        ) : (
          <SaldoLogo className="h-7 w-7" aria-label="Saldo" />
        )}
      </div>

      {/* Collapse toggle */}
      <button
        className="absolute right-0 top-[1.125rem] translate-x-1/2 z-10 h-6 w-6 rounded-full bg-card border border-border shadow-sm flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
        onClick={() => {
          onToggleCollapse()
          setHovered(false)
        }}
        aria-label={collapsed ? t('sidebar.expand') : t('sidebar.collapse')}
      >
        {collapsed ? <ChevronsRight className="h-3 w-3" /> : <ChevronsLeft className="h-3 w-3" />}
      </button>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto overflow-x-hidden px-2 pb-3">
        <div className="flex flex-col gap-1">{MONEY_ITEMS.map(renderItem)}</div>
        <div className="mt-3 flex flex-col gap-1 border-t border-border pt-3">{SETUP_ITEMS.map(renderItem)}</div>
        <div className="mt-auto flex flex-col gap-1 pt-3">{renderItem(SETTINGS_ITEM)}</div>
      </div>
    </nav>
  )
}

export default Sidebar
