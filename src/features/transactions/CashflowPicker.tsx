import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { Check, ChevronDown } from 'lucide-react'
import { listEligibleCashflows } from '../../shared/api'
import type { EventWithData } from '../../shared/types'
import { Popover, PopoverContent, PopoverTrigger } from '../../shared/ui/popover'
import { Button } from '../../shared/ui/button'
import NumberValue from '../../shared/ui/NumberValue'
import { cn } from '@/shared/lib/utils'

interface CashflowPickerProps {
  personId: number
  eligibleAmountMinor?: number
  selectedIds?: number[]
  onToggle?: (id: number) => void
  onLink?: (id: number) => void
  currencyMinorUnits: number
  currencyCode: string
}

const CashflowPicker = ({ personId, eligibleAmountMinor, selectedIds, onToggle, onLink, currencyMinorUnits, currencyCode }: CashflowPickerProps) => {
  const { t } = useTranslation()

  const cashflowsQuery = useQuery({
    queryKey: ['eligible-cashflows', personId],
    queryFn: () => listEligibleCashflows(personId, undefined, true),
  })

  const allCashflows: EventWithData[] = cashflowsQuery.data ?? []

  const eligible = eligibleAmountMinor !== undefined ? allCashflows.filter((cf) => cf.amountMinor === eligibleAmountMinor) : []
  const other = eligibleAmountMinor !== undefined ? allCashflows.filter((cf) => cf.amountMinor !== eligibleAmountMinor) : allCashflows

  const selectedCount = selectedIds?.length ?? 0
  const triggerLabel = selectedCount > 0 ? t('taxable.cashflowPickerCount', { count: selectedCount }) : t('taxable.cashflowPickerTrigger')

  const handleClick = (id: number) => {
    if (onToggle) {
      onToggle(id)
    } else if (onLink) {
      onLink(id)
    }
  }

  const renderItem = (cf: EventWithData) => {
    const selected = selectedIds?.includes(cf.id) ?? false
    return (
      <button
        key={cf.id}
        type="button"
        onClick={() => handleClick(cf.id)}
        className={cn(
          'w-full flex flex-col px-2 py-1.5 rounded-md text-sm transition-colors text-left',
          selected ? 'bg-accent text-accent-foreground' : 'hover:bg-muted text-foreground',
        )}
      >
        <div className="flex items-center justify-between gap-2">
          <span>{cf.eventDate.slice(0, 10)}</span>
          <div className="flex items-center gap-1">
            <NumberValue value={cf.amountMinor} minorUnits={currencyMinorUnits} currencyCode={currencyCode} className="font-medium tabular-nums" />
            <Check className={cn('size-4 shrink-0', selected ? 'opacity-100' : 'opacity-0')} />
          </div>
        </div>
        {cf.note && <span className="text-xs text-muted-foreground truncate">{cf.note}</span>}
      </button>
    )
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button type="button" variant="outline" className="w-full justify-between gap-2">
          <span>{triggerLabel}</span>
          <ChevronDown className="size-4 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[var(--radix-popover-trigger-width)] p-2" onOpenAutoFocus={(e) => e.preventDefault()}>
        <div className="max-h-72 overflow-y-auto flex flex-col gap-1" onWheel={(e) => e.stopPropagation()}>
          {cashflowsQuery.isLoading ? (
            <p className="text-xs text-muted-foreground px-2 py-1">{t('common.loading')}</p>
          ) : allCashflows.length === 0 ? (
            <p className="text-xs text-muted-foreground px-2 py-1">{t('taxable.noEligibleCashflows')}</p>
          ) : (
            <>
              {eligible.length > 0 && (
                <div>
                  <p className="text-xs font-bold text-foreground uppercase tracking-wide px-2 mb-1">{t('taxable.cashflowPickerEligible')}</p>
                  {eligible.map((cf) => renderItem(cf))}
                </div>
              )}
              {eligible.length > 0 && other.length > 0 && <div className="border-t my-1" />}
              {other.length > 0 && (
                <div>
                  {eligible.length > 0 && <p className="text-xs font-bold text-foreground uppercase tracking-wide px-2 mb-1">{t('taxable.cashflowPickerOther')}</p>}
                  {other.map((cf) => renderItem(cf))}
                </div>
              )}
            </>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

export default CashflowPicker
