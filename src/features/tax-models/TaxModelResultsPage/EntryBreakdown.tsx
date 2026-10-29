import { useTranslation } from 'react-i18next'
import NumberValue from '@/shared/ui/NumberValue'
import { ENTRY_KINDS, type EntryKind, type TaxEventBreakdown } from '@/shared/types'

interface EntryBreakdownProps {
  rows: TaxEventBreakdown[]
  minorUnits: number
  currencyCode?: string
}

type AmountKey = 'amountMinor' | 'netAmountMinor' | 'vatAmountMinor' | 'reclaimableVatMinor' | 'taxDeductibleCostMinor' | 'nonTaxDeductibleCostMinor'

const AMOUNT_COLUMNS: { key: AmountKey; label: string }[] = [
  { key: 'amountMinor', label: 'taxModels.breakdownAmount' },
  { key: 'netAmountMinor', label: 'taxModels.breakdownNetAmount' },
  { key: 'vatAmountMinor', label: 'taxModels.breakdownVat' },
  { key: 'reclaimableVatMinor', label: 'taxModels.breakdownReclaimableVat' },
  { key: 'taxDeductibleCostMinor', label: 'taxModels.breakdownDeductibleCost' },
  { key: 'nonTaxDeductibleCostMinor', label: 'taxModels.breakdownNonDeductibleCost' },
]

const sumColumn = (rows: TaxEventBreakdown[], key: AmountKey): number => rows.reduce((sum, row) => sum + row[key], 0)

interface GroupRowsProps {
  kind: EntryKind
  rows: TaxEventBreakdown[]
  minorUnits: number
}

const GroupRows = ({ kind, rows, minorUnits }: GroupRowsProps) => {
  const { t } = useTranslation()
  const counted = rows.filter((row) => row.notCounted === null)

  return (
    <>
      <tr className="bg-muted/50">
        <td colSpan={3 + AMOUNT_COLUMNS.length} className="py-1 px-1 font-semibold text-foreground">
          {t(`taxModels.forecast.kind.${kind}`)}
        </td>
      </tr>
      {rows.map((row) => (
        <tr key={row.eventId} className={row.notCounted === null ? 'border-b border-border/40' : 'border-b border-border/40 opacity-50'}>
          <td className="py-1.5 pr-3 whitespace-nowrap">{row.eventDate.slice(0, 10)}</td>
          <td className="py-1.5 pr-3 text-muted-foreground max-w-[12rem] truncate">{row.note ?? ''}</td>
          <td className="py-1.5 pr-3 whitespace-nowrap">
            {t(`taxModels.forecast.kind.${row.kind}`)}
            {row.notCounted !== null && <div className="text-[10px] italic">{t(`taxModels.forecast.reason.${row.notCounted}`)}</div>}
          </td>
          {AMOUNT_COLUMNS.map((column) => (
            <td key={column.key} className="py-1.5 pl-3 text-right">
              <NumberValue value={row[column.key]} minorUnits={minorUnits} />
            </td>
          ))}
        </tr>
      ))}
      <tr className="border-b border-border font-semibold">
        <td colSpan={3} className="py-1.5 pr-3 text-muted-foreground">
          {t('taxModels.breakdownTotal')}
        </td>
        {AMOUNT_COLUMNS.map((column) => (
          <td key={column.key} className="py-1.5 pl-3 text-right">
            <NumberValue value={sumColumn(counted, column.key)} minorUnits={minorUnits} />
          </td>
        ))}
      </tr>
    </>
  )
}

export const EntryBreakdown = ({ rows, minorUnits, currencyCode }: EntryBreakdownProps) => {
  const { t } = useTranslation()
  const groups = ENTRY_KINDS.map((kind) => ({ kind, rows: rows.filter((row) => row.kind === kind) })).filter((group) => group.rows.length > 0)

  return (
    <details className="mt-6 text-sm">
      <summary className="cursor-pointer select-none text-muted-foreground hover:text-foreground py-2">
        {t('taxModels.breakdownSection')}
        {currencyCode && ` (${currencyCode})`}
      </summary>
      <div className="mt-3 overflow-x-auto">
        <table className="w-full text-xs border-collapse whitespace-nowrap">
          <thead>
            <tr className="border-b border-border">
              <th className="text-left py-2 pr-3 font-semibold text-muted-foreground">{t('taxModels.breakdownDate')}</th>
              <th className="text-left py-2 pr-3 font-semibold text-muted-foreground">{t('taxModels.breakdownNote')}</th>
              <th className="text-left py-2 pr-3 font-semibold text-muted-foreground">{t('taxModels.breakdownType')}</th>
              {AMOUNT_COLUMNS.map((column) => (
                <th key={column.key} className="text-right py-2 pl-3 font-semibold text-muted-foreground">
                  {t(column.label)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {groups.map((group) => (
              <GroupRows key={group.kind} kind={group.kind} rows={group.rows} minorUnits={minorUnits} />
            ))}
          </tbody>
        </table>
      </div>
    </details>
  )
}
