import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import type { AccountKind, CountedLine, Currency, LinePart, LineSubject, Metric, RateUsed } from '../../shared/types'
import { cn } from '@/shared/lib/utils'
import NumberValue from '../../shared/ui/NumberValue'
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../../shared/ui/dialog'
import { formatDisplayDate } from '../../shared/utils/format'
import { formatRate } from '../../shared/utils/fxRate'

interface Props {
  metric: Metric
  currency: Currency
  onClose: () => void
}

const partLabel = (part: LinePart, accountType: AccountKind, t: TFunction): string | null => {
  switch (part.kind) {
    case 'balance':
      return accountType === 'bucket' ? t('buckets.additionalBalance') : null
    case 'linkedAccount':
      return t('metrics.breakdown.part.linkedAccount', { account: part.sourceAccountName })
    case 'taggedCashflows':
      return t('metrics.breakdown.part.taggedCashflows', { account: part.sourceAccountName })
    case 'bucketTotal':
      return t('metrics.breakdown.part.bucketTotal')
    default: {
      const _exhaustive: never = part
      return _exhaustive
    }
  }
}

const lineKey = ({ accountId, part }: LineSubject) => {
  const source = part.kind === 'linkedAccount' || part.kind === 'taggedCashflows' ? part.sourceAccountId : ''
  return `${accountId}:${part.kind}:${source}`
}

const AccountLabel = ({ subject }: { subject: LineSubject }) => {
  const { t } = useTranslation()
  const sublabel = partLabel(subject.part, subject.accountType, t)
  return (
    <div className="flex flex-col">
      <span className="font-medium">{subject.accountName}</span>
      {sublabel && <span className="text-xs text-muted-foreground">{sublabel}</span>}
    </div>
  )
}

const RateCell = ({ rate, heldCode, consolidationCode }: { rate: RateUsed | null; heldCode: string; consolidationCode: string }) => {
  const { t, i18n } = useTranslation()
  if (!rate) return <span className="text-muted-foreground">{t('metrics.breakdown.noConversion')}</span>

  const value = formatRate(rate.mantissa, rate.exponent)
  const quote = rate.isDirect ? `1 ${heldCode} = ${value} ${consolidationCode}` : `1 ${consolidationCode} = ${value} ${heldCode}`

  return (
    <div className="flex flex-col">
      <span className="tabular-nums">{quote}</span>
      <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
        {formatDisplayDate(rate.date, i18n.language)}
        {rate.isManual && <span className="rounded bg-muted px-1.5 py-0.5">{t('metrics.breakdown.manual')}</span>}
      </span>
      {rate.stale && <span className="text-xs font-medium text-amber-700 dark:text-amber-400">{t('metrics.breakdown.rateAge', { count: rate.ageDays })}</span>}
    </div>
  )
}

const SignedValue = ({ line, currency }: { line: CountedLine; currency: Currency }) => (
  <span className={cn('tabular-nums', line.signedMinor < 0 && 'text-destructive')}>
    {line.signedMinor > 0 && '+'}
    <NumberValue value={line.signedMinor} currencyCode={currency.code} minorUnits={currency.minorUnits} />
  </span>
)

const MetricBreakdownDialog = ({ metric, currency, onClose }: Props) => {
  const { t } = useTranslation()
  const countedHeadingId = useId()
  const skippedHeadingId = useId()

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-baseline gap-3 pr-6">
            <span>{t(`metrics.${metric.kind}`)}</span>
            <NumberValue value={metric.totalMinor} currencyCode={currency.code} minorUnits={currency.minorUnits} className={cn(metric.totalMinor < 0 && 'text-destructive')} />
          </DialogTitle>
          <DialogDescription>{t('metrics.breakdown.description')}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-6">
          <section>
            <h3 id={countedHeadingId} className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t('metrics.breakdown.counted')}
            </h3>
            <table aria-labelledby={countedHeadingId} className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-muted-foreground">
                  <th scope="col" className="pb-2 pr-4 font-normal">
                    {t('metrics.breakdown.columns.account')}
                  </th>
                  <th scope="col" className="pb-2 pr-4 text-right font-normal">
                    {t('metrics.breakdown.columns.amount')}
                  </th>
                  <th scope="col" className="pb-2 pr-4 font-normal">
                    {t('metrics.breakdown.columns.rate')}
                  </th>
                  <th scope="col" className="pb-2 text-right font-normal">
                    {t('metrics.breakdown.columns.value', { currency: currency.code })}
                  </th>
                </tr>
              </thead>
              <tbody>
                {metric.counted.length === 0 && (
                  <tr>
                    <td colSpan={4} className="py-3 text-muted-foreground">
                      {t('metrics.breakdown.nothingCounted')}
                    </td>
                  </tr>
                )}
                {metric.counted.map((line) => (
                  <tr key={lineKey(line)} className="align-top">
                    <td className="py-1.5 pr-4">
                      <AccountLabel subject={line} />
                    </td>
                    <td className="py-1.5 pr-4 text-right">
                      <NumberValue value={line.native.amountMinor} currencyCode={line.native.currencyCode} minorUnits={line.native.minorUnits} className="tabular-nums" />
                    </td>
                    <td className="py-1.5 pr-4">
                      <RateCell rate={line.rate} heldCode={line.native.currencyCode} consolidationCode={currency.code} />
                    </td>
                    <td className="py-1.5 text-right">
                      <SignedValue line={line} currency={currency} />
                    </td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr className="border-t border-border font-semibold">
                  <th scope="row" colSpan={3} className="pt-2 text-left">
                    {t('metrics.breakdown.total')}
                  </th>
                  <td className="pt-2 text-right">
                    <NumberValue
                      value={metric.totalMinor}
                      currencyCode={currency.code}
                      minorUnits={currency.minorUnits}
                      className={cn('tabular-nums', metric.totalMinor < 0 && 'text-destructive')}
                    />
                  </td>
                </tr>
              </tfoot>
            </table>
          </section>

          {metric.skipped.length > 0 && (
            <section>
              <h3 id={skippedHeadingId} className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {t('metrics.breakdown.notCounted')}
              </h3>
              <ul aria-labelledby={skippedHeadingId} className="flex flex-col gap-3 text-sm">
                {metric.skipped.map((line) => (
                  <li key={lineKey(line)} className="flex items-start justify-between gap-4">
                    <div className="flex flex-col">
                      <AccountLabel subject={line} />
                      <span className="text-xs text-muted-foreground">{t(`metrics.breakdown.reason.${line.reason}`, { currency: line.amount.currencyCode })}</span>
                    </div>
                    <NumberValue value={line.amount.amountMinor} currencyCode={line.amount.currencyCode} minorUnits={line.amount.minorUnits} className="shrink-0 tabular-nums" />
                  </li>
                ))}
              </ul>
            </section>
          )}
        </DialogBody>
      </DialogContent>
    </Dialog>
  )
}

export default MetricBreakdownDialog
