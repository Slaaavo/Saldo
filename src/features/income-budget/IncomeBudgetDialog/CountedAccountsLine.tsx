import { useTranslation } from 'react-i18next'
import type { CountedAccount } from '@/shared/types'

interface Props {
  accounts: CountedAccount[]
  personName: string
  currencyCode: string
}

export const CountedAccountsLine = ({ accounts, personName, currencyCode }: Props) => {
  const { t, i18n } = useTranslation()
  const text =
    accounts.length === 0
      ? t('incomeBudget.counted.none', { person: personName, currency: currencyCode })
      : t('incomeBudget.counted.some', { accounts: new Intl.ListFormat(i18n.language, { type: 'conjunction' }).format(accounts.map((account) => account.name)) })
  return <p className="text-xs text-muted-foreground">{text}</p>
}
