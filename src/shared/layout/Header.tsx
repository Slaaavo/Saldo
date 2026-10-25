import { useTranslation } from 'react-i18next'
import { useRouterState } from '@tanstack/react-router'
import { cn } from '@/shared/lib/utils'
import { DatePicker } from '../ui/date-picker'
import { Label } from '../ui/label'
import { useTimeMachine } from '../../app/useTimeMachine'
import PersonSelector from '../../features/dashboard/PersonSelector'

interface Props {
  pageTitle: string
}

const Header = ({ pageTitle }: Props) => {
  const { t } = useTranslation()
  const { asOf, travelTo, backToLive } = useTimeMachine()
  const routerState = useRouterState()
  const pathname = routerState.location.pathname
  const showDatePicker = pathname === '/dashboard'
  const showPersonSelector = pathname === '/dashboard' || pathname === '/ledger'

  return (
    <header className="flex items-center justify-between border-b bg-card px-4 md:px-10 py-3 min-h-16">
      <div className="flex items-center">
        <h1 className="text-xl font-bold tracking-tight">{pageTitle}</h1>
      </div>
      <div className="flex items-center gap-3">
        {showPersonSelector && <PersonSelector />}
        {showDatePicker && (
          <>
            <Label htmlFor="date-picker" className="text-sm font-medium text-muted-foreground">
              {t('header.date')}
            </Label>
            <DatePicker
              id="date-picker"
              value={asOf.kind === 'at' ? asOf.until : undefined}
              onChange={(date) => (date ? travelTo(date) : backToLive())}
              placeholder={t('timeMachine.live')}
              clearable
              showToday={false}
              className={cn('w-48', asOf.kind === 'at' && 'border-amber-500 text-amber-800 dark:border-amber-500/70 dark:text-amber-300')}
            />
          </>
        )}
      </div>
    </header>
  )
}

export default Header
