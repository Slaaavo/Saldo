import { useState } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import type { BankLine, DifferenceCause, RowTarget, SnapshotRow } from '@/shared/types'
import { useModal } from '@/app/useModal'
import { Button } from '@/shared/ui/button'
import ConfirmDialog from '@/shared/ui/ConfirmDialog'
import NumberValue from '@/shared/ui/NumberValue'
import { formatShortDay } from '@/shared/utils/format'
import { useFixDifferenceCause, type RowCause } from './useBankCheckMutations'

type StaleImportCause = Extract<DifferenceCause, { kind: 'staleImport' }>

interface Props {
  row: SnapshotRow
  causes: DifferenceCause[]
}

const FIX_BUTTONS: Record<RowCause['kind'], string> = {
  bookedDifferenceExplained: 'bankCheck.causes.deleteBookedDifference',
  nearCopy: 'bankCheck.causes.deleteCopy',
  equalsGap: 'bankCheck.causes.deleteRow',
  flippedSign: 'bankCheck.causes.flipSign',
}

const DELETE_MESSAGES: Record<RowTarget['kind'], string> = {
  cashflow: 'bankCheck.causes.confirm.deleteRow',
  unexplainedDifference: 'bankCheck.causes.confirm.deleteBookedDifference',
  transfer: 'bankCheck.causes.confirm.deleteTransfer',
  splitGroup: 'bankCheck.causes.confirm.deleteSplitGroup',
}

const isRowCause = (cause: DifferenceCause): cause is RowCause => cause.kind !== 'staleImport'

const isStaleImport = (cause: DifferenceCause): cause is StaleImportCause => cause.kind === 'staleImport'

const dayOf = (date: string, language: string) => formatShortDay(date.slice(0, 10), language)

const titleDay = (cause: RowCause) => {
  if (cause.kind === 'nearCopy') return cause.copyOf.eventDate
  if (cause.kind === 'bookedDifferenceExplained') return cause.row.eventDate
  return null
}

const rowLabel = (line: BankLine, language: string) => [dayOf(line.eventDate, language), line.note ?? line.counterpartName].filter(Boolean).join(' · ')

const causeKey = (cause: RowCause) => {
  const { target } = cause.row
  return `${cause.kind}-${target.kind === 'splitGroup' ? `group${target.splitGroupId}` : `event${target.eventId}`}`
}

const Money = ({ row, value }: { row: SnapshotRow; value: number }) => (
  <NumberValue value={value} currencyCode={row.currencyCode} minorUnits={row.currencyMinorUnits} className="font-semibold" />
)

const StaleImportLine = ({ cause, row }: { cause: StaleImportCause; row: SnapshotRow }) => {
  const { t, i18n } = useTranslation()
  const { setModalState } = useModal()
  return (
    <li className="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
      <span>{cause.newestDay ? t('bankCheck.causes.staleImport', { day: dayOf(cause.newestDay, i18n.language) }) : t('bankCheck.causes.noImportedRows')}</span>
      <Button type="button" variant="outline" size="sm" onClick={() => setModalState({ type: 'csvImport', preselectedAccountId: row.accountId })}>
        {t('bankCheck.causes.importCsv')}
      </Button>
    </li>
  )
}

const CauseLine = ({ cause, row, onFix }: { cause: RowCause; row: SnapshotRow; onFix: () => void }) => {
  const { t, i18n } = useTranslation()
  const day = titleDay(cause)
  const title = day ? t(`bankCheck.causes.${cause.kind}`, { day: dayOf(day, i18n.language) }) : t(`bankCheck.causes.${cause.kind}`)
  return (
    <li className="flex items-center justify-between gap-3 rounded-md border px-3 py-2">
      <div className="flex min-w-0 flex-col">
        <span>{title}</span>
        <span className="flex min-w-0 text-muted-foreground">
          <span className="truncate">{rowLabel(cause.row, i18n.language)}</span>
          <span className="shrink-0 whitespace-pre">
            {' · '}
            <Money row={row} value={cause.row.amountMinor} />
          </span>
        </span>
      </div>
      <Button type="button" variant="outline" size="sm" onClick={onFix}>
        {t(FIX_BUTTONS[cause.kind])}
      </Button>
    </li>
  )
}

const RowLabel = ({ line, language }: { line: BankLine; language: string }) => <span>{rowLabel(line, language)}</span>

const FixMessage = ({ cause, row }: { cause: RowCause; row: SnapshotRow }) => {
  const { i18n } = useTranslation()
  const { target, amountMinor } = cause.row
  const label = <RowLabel line={cause.row} language={i18n.language} />
  const amount = <Money row={row} value={amountMinor} />
  if (cause.kind === 'flippedSign') {
    return <Trans i18nKey="bankCheck.causes.confirm.flip" components={{ row: label, amount, flipped: <Money row={row} value={-amountMinor} /> }} />
  }
  const values = target.kind === 'splitGroup' ? { parts: target.parts } : undefined
  return <Trans i18nKey={DELETE_MESSAGES[target.kind]} values={values} components={{ row: label, amount }} />
}

const DifferenceCauses = ({ row, causes }: Props) => {
  const { t } = useTranslation()
  const fix = useFixDifferenceCause()
  const [confirming, setConfirming] = useState<RowCause | null>(null)
  const staleImport = causes.find(isStaleImport)
  const rowCauses = causes.filter(isRowCause)

  return (
    <section className="flex flex-col gap-2 text-sm">
      <h3 className="font-medium">{t('bankCheck.causes.title')}</h3>
      {causes.length > 0 && (
        <ul className="flex flex-col gap-2">
          {staleImport && <StaleImportLine cause={staleImport} row={row} />}
          {rowCauses.map((cause) => (
            <CauseLine key={causeKey(cause)} cause={cause} row={row} onFix={() => setConfirming(cause)} />
          ))}
        </ul>
      )}
      {rowCauses.length === 0 && <p className="text-muted-foreground">{t('bankCheck.causes.none')}</p>}
      {confirming && (
        <ConfirmDialog
          action={confirming.kind === 'flippedSign' ? 'confirm' : 'delete'}
          message={<FixMessage cause={confirming} row={row} />}
          loading={fix.isPending}
          onConfirm={() => fix.mutate(confirming, { onSettled: () => setConfirming(null) })}
          onCancel={() => setConfirming(null)}
        />
      )}
    </section>
  )
}

export default DifferenceCauses
