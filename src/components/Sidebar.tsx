import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  LayoutDashboard,
  Settings,
  ArrowLeftRight,
  ChevronsLeft,
  ChevronsRight,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import SaldoLogo from '@/components/SaldoLogo';
import saldoLogotype from '@/assets/Saldo logotype transparent.svg';
import saldoLogotypeDark from '@/assets/Saldo logotype transparent dark.svg';

interface SidebarProps {
  currentView: 'dashboard' | 'fx-rates' | 'settings';
  onNavigate: (view: 'dashboard' | 'fx-rates' | 'settings') => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
}

export default function Sidebar({
  currentView,
  onNavigate,
  collapsed,
  onToggleCollapse,
}: SidebarProps) {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false);

  const expanded = !collapsed || (collapsed && hovered);

  const navItems = [
    { view: 'dashboard' as const, icon: LayoutDashboard, label: t('sidebar.dashboard') },
    { view: 'settings' as const, icon: Settings, label: t('sidebar.settings') },
    { view: 'fx-rates' as const, icon: ArrowLeftRight, label: t('sidebar.fxRates') },
  ];

  return (
    <nav
      className={cn(
        'group relative flex flex-col bg-card border-r border-border h-screen shrink-0 transition-[width] duration-150 ease-in-out',
        expanded ? 'w-56' : 'w-14',
      )}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* App name row */}
      <div className="flex items-center justify-center px-4 py-3 h-14">
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
          onToggleCollapse();
          setHovered(false);
        }}
        aria-label={collapsed ? t('sidebar.expand') : t('sidebar.collapse')}
      >
        {collapsed ? <ChevronsRight className="h-3 w-3" /> : <ChevronsLeft className="h-3 w-3" />}
      </button>

      {/* Nav items */}
      <div className="flex flex-col gap-1 px-2">
        {navItems.map(({ view, icon: Icon, label }) => (
          <button
            key={view}
            onClick={() => onNavigate(view)}
            className={cn(
              'w-full flex items-center gap-3 px-2 py-2 rounded-lg text-sm font-medium transition-colors',
              currentView === view
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground',
            )}
          >
            <Icon className="h-5 w-5 shrink-0" />
            <span
              className={cn(
                'overflow-hidden whitespace-nowrap transition-opacity duration-150',
                expanded ? 'opacity-100' : 'opacity-0 w-0',
              )}
            >
              {label}
            </span>
          </button>
        ))}
      </div>
    </nav>
  );
}
