import { useTranslation } from 'react-i18next'
import type { IbanSegment } from '../../../shared/utils/formatIban'
import { formatIbanSegments } from '../../../shared/utils/formatIban'

interface IbanDisplayProps {
  iban: string | null | undefined
}

export const IbanDisplay = ({ iban }: IbanDisplayProps) => {
  const { t } = useTranslation()

  const trimmed = iban?.trim()

  if (!trimmed) {
    return <span className="text-xs text-muted-foreground">{t('accounts.noIban')}</span>
  }

  return (
    <p className="text-xs text-muted-foreground truncate" title={trimmed.replace(/(.{4})/g, '$1 ').trim()}>
      {formatIbanSegments(trimmed).map((seg: IbanSegment, i: number) => (
        <span key={i} className={seg.weight}>
          {seg.text}
        </span>
      ))}
    </p>
  )
}
