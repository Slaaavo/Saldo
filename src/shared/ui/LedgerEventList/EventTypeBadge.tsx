import { useTranslation } from 'react-i18next'
import { ArrowUpDown, Receipt, Scale, TrendingUp, TrendingDown } from 'lucide-react'
import type { EventType } from '../../types'

interface Props {
  eventType: EventType
}

const EventTypeBadge = ({ eventType }: Props) => {
  const { t } = useTranslation()

  if (eventType === 'transfer') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
        <ArrowUpDown className="h-3 w-3" />
        {t('events.type.transfer')}
      </span>
    )
  }

  if (eventType === 'cashflow') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
        <Receipt className="h-3 w-3" />
        {t('events.type.cashflow')}
      </span>
    )
  }

  if (eventType === 'revenue') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">
        <TrendingUp className="h-3 w-3" />
        {t('events.type.revenue')}
      </span>
    )
  }

  if (eventType === 'expense') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-rose-100 px-2 py-0.5 text-xs font-medium text-rose-700 dark:bg-rose-900/30 dark:text-rose-400">
        <TrendingDown className="h-3 w-3" />
        {t('events.type.expense')}
      </span>
    )
  }

  if (eventType === 'prepaid_expense') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-rose-100 px-2 py-0.5 text-xs font-medium text-rose-700 dark:bg-rose-900/30 dark:text-rose-400">
        <TrendingDown className="h-3 w-3" />
        {t('events.type.prepaid_expense')}
      </span>
    )
  }

  if (eventType === 'unexplained_difference') {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-orange-100 px-2 py-0.5 text-xs font-medium text-orange-700 dark:bg-orange-900/30 dark:text-orange-400">
        <Scale className="h-3 w-3" />
        {t('events.type.unexplained_difference')}
      </span>
    )
  }

  return null
}

export default EventTypeBadge
