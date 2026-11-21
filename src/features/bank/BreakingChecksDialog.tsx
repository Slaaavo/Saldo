import { Trans, useTranslation } from 'react-i18next'
import type { BrokenCheck } from '@/shared/types'
import ConfirmDialog from '@/shared/ui/ConfirmDialog'
import NumberValue from '@/shared/ui/NumberValue'
import { formatShortDay } from '@/shared/utils/format'

interface Props {
  checks: BrokenCheck[]
  onConfirm: () => void
  onCancel: () => void
}

const AccountName = ({ name }: { name: string }) => <span className="font-semibold">{name}</span>

const CheckLine = ({ check }: { check: BrokenCheck }) => {
  const { i18n } = useTranslation()
  const amount = <NumberValue value={Math.abs(check.differenceMinor)} currencyCode={check.currencyCode} minorUnits={check.currencyMinorUnits} className="font-semibold" />
  return (
    <span className="block">
      <Trans
        i18nKey="bankCheck.breaks.line"
        values={{ day: formatShortDay(check.day, i18n.language) }}
        components={{ account: <AccountName name={check.accountName} />, amount }}
      />
    </span>
  )
}

const BreakingChecksDialog = ({ checks, onConfirm, onCancel }: Props) => (
  <ConfirmDialog
    action="breakChecks"
    message={
      <span className="flex flex-col gap-2">
        {checks.map((check) => (
          <CheckLine key={check.accountId} check={check} />
        ))}
      </span>
    }
    onConfirm={onConfirm}
    onCancel={onCancel}
  />
)

export default BreakingChecksDialog
