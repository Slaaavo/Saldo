import { useTranslation } from 'react-i18next'
import { useRouterState } from '@tanstack/react-router'
import { DatePicker } from '../ui/date-picker'
import { Label } from '../ui/label'
import { useSelectedDate } from '../../app/useSelectedDate'

interface Props {
  pageTitle: string
}

const Header = ({ pageTitle }: Props) => {
  const { t } = useTranslation()
  const { selectedDate, setSelectedDate } = useSelectedDate()
  const routerState = useRouterState()
  const showDatePicker = routerState.location.pathname === '/dashboard'

  return (
    <header className="flex items-center justify-between border-b bg-card px-4 md:px-10 py-3 min-h-16">
      <div className="flex items-center">
        <h1 className="text-xl font-bold tracking-tight">{pageTitle}</h1>
      </div>
      <div className="flex items-center gap-2">
        {showDatePicker && (
          <>
            <Label htmlFor="date-picker" className="text-sm font-medium text-muted-foreground">
              {t('header.date')}
            </Label>
            <DatePicker id="date-picker" value={selectedDate} onChange={setSelectedDate} className="w-48" />
          </>
        )}
      </div>
    </header>
  )
}

export default Header
