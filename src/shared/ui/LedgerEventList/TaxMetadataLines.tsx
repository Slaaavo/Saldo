import { useTranslation } from 'react-i18next'

interface Props {
  eventType: string
  vatRateBps: number | null
  vatReclaimablePctBps: number | null
  expenseDeductiblePctBps: number | null
  prepaidUntil: string | null
  reclaimedVat: boolean | null
}

const TaxMetadataLines = ({ eventType, vatRateBps, vatReclaimablePctBps, expenseDeductiblePctBps, prepaidUntil, reclaimedVat }: Props) => {
  const { t } = useTranslation()

  const hasVatRate = vatRateBps !== null
  const hasVatReclaimable = vatReclaimablePctBps !== null && vatReclaimablePctBps < 10000
  const hasExpenseDeductible = expenseDeductiblePctBps !== null && expenseDeductiblePctBps < 10000
  const hasPrepaidUntil = !!prepaidUntil
  const hasReclaimedVat = eventType === 'expense' && reclaimedVat === true

  if (!hasVatRate && !hasVatReclaimable && !hasExpenseDeductible && !hasPrepaidUntil && !hasReclaimedVat) {
    return null
  }

  return (
    <>
      {hasVatRate && <p className="text-xs text-muted-foreground">{t('events.vatRate', { rate: vatRateBps! / 100 })}</p>}
      {hasVatReclaimable && <p className="text-xs text-muted-foreground">{t('events.vatReclaimable', { pct: vatReclaimablePctBps! / 100 })}</p>}
      {hasExpenseDeductible && <p className="text-xs text-muted-foreground">{t('events.expenseDeductible', { pct: expenseDeductiblePctBps! / 100 })}</p>}
      {hasPrepaidUntil && <p className="text-xs text-muted-foreground">{t('events.prepaidUntil', { date: prepaidUntil!.slice(0, 10) })}</p>}
      {hasReclaimedVat && <span className="inline-flex items-center rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground mt-0.5">{t('events.reclaimedVat')}</span>}
    </>
  )
}

export default TaxMetadataLines
