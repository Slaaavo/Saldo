import { useTranslation } from 'react-i18next'
import { Button } from '../../shared/ui/button'
import { formatDisplayDate } from '../../shared/utils/format'

interface Props {
  until: string
  onBackToLive: () => void
}

const TimeMachineBanner = ({ until, onBackToLive }: Props) => {
  const { t, i18n } = useTranslation()

  return (
    <div role="status" className="flex items-center justify-between gap-4 border-b border-amber-300 dark:border-amber-700/50 bg-amber-100 dark:bg-amber-900/40 px-4 md:px-10 py-2">
      <span className="text-sm text-amber-800 dark:text-amber-300">{t('timeMachine.viewing', { date: formatDisplayDate(until.substring(0, 10), i18n.language) })}</span>
      <Button
        variant="outline"
        size="sm"
        onClick={onBackToLive}
        className="shrink-0 border-amber-400 text-amber-800 hover:bg-amber-200 dark:border-amber-600 dark:text-amber-300 dark:hover:bg-amber-800/40"
      >
        {t('timeMachine.backToLive')}
      </Button>
    </div>
  )
}

export default TimeMachineBanner
