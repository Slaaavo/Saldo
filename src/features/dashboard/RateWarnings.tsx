import { useTranslation } from 'react-i18next'
import type { RateWarning } from '../../shared/types'
import NumberValue from '../../shared/ui/NumberValue'
import { formatDisplayDate } from '../../shared/utils/format'

interface Props {
  warnings: RateWarning[]
}

const RateWarnings = ({ warnings }: Props) => {
  const { t, i18n } = useTranslation()

  return (
    <ul className="mx-4 md:mx-10 mt-4 flex flex-col gap-1 rounded-md border border-amber-300 bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:border-amber-500/40 dark:bg-amber-900/20 dark:text-amber-300">
      {warnings.map(({ accountId, accountName, native, problem }) => (
        <li key={accountId}>
          <span className="font-medium">{accountName}</span> (<NumberValue value={native.amountMinor} currencyCode={native.currencyCode} minorUnits={native.minorUnits} />){' '}
          {problem.kind === 'noRate'
            ? t('rateWarnings.noRate', { currency: native.currencyCode })
            : t('rateWarnings.stale', {
                currency: native.currencyCode,
                date: formatDisplayDate(problem.rate.date, i18n.language),
                count: problem.rate.ageDays,
              })}
        </li>
      ))}
    </ul>
  )
}

export default RateWarnings
