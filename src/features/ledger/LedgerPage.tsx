import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import type { EventWithData } from '../../shared/types'
import { getUnmatchedCashflowCount } from '../../shared/api'
import { useLedgerData } from './useLedgerData'
import LedgerEventList from '../../shared/ui/LedgerEventList'
import PortfolioItemFilter from './PortfolioItemFilter'
import { DatePicker } from '../../shared/ui/date-picker'
import { Button } from '../../shared/ui/button'
import LedgerActionButtons from '../../shared/ui/LedgerActionButtons'
import { useSnapshotQuery } from '../../shared/hooks/useSnapshotQuery'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useEditEventHandler } from '../../shared/hooks/useEditEventHandler'
import { useModal } from '../../app/useModal'
import { LIVE } from '../../shared/utils/snapshot'
import { useSelectedPerson } from '../../app/useSelectedPerson'

const LedgerPage = () => {
  const { t } = useTranslation()
  const { setModalState } = useModal()
  const { selectedPersonId } = useSelectedPerson()
  const handleEditEvent = useEditEventHandler()
  const snapshotQuery = useSnapshotQuery(LIVE)
  const snapshot = snapshotQuery.data?.rows ?? []
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const consolidationCurrency = consolidationCurrencyQuery.data ?? null

  const unmatchedCountQuery = useQuery({
    queryKey: ['events', 'unmatched-cashflow-count', selectedPersonId],
    queryFn: () => getUnmatchedCashflowCount(selectedPersonId ?? undefined),
  })
  const unmatchedCount = unmatchedCountQuery.data ?? 0

  const {
    fromDate,
    setFromDate,
    toDate,
    setToDate,
    selectedAccountIds,
    setSelectedAccountIds,
    eventTypeFilter,
    setEventTypeFilter,
    unmatchedOnly,
    setUnmatchedOnly,
    events,
    loading,
  } = useLedgerData({
    snapshot,
  })

  const handleEditTaxableSplitGroup = (splitGroupId: number, eventType: string, legs: EventWithData[], groupNote: string | null, accountId: number) => {
    setModalState({ type: 'editTaxableSplitGroup', splitGroupId, eventType: eventType as 'revenue' | 'expense', legs, groupNote, accountId })
  }

  const handleDeleteEvent = (eventId: number, eventType?: string) => {
    setModalState({ type: 'confirmDeleteEvent', eventId, eventType })
  }

  const handleDeleteSplitGroup = (splitGroupId: number) => {
    setModalState({ type: 'confirmDeleteSplitGroup', splitGroupId })
  }

  const handleDeleteTransferEvent = (eventId: number, linkedEventId: number) => {
    setModalState({ type: 'confirmDeleteTransferEvent', eventId, linkedEventId })
  }

  const handleAddEvents = () => {
    setModalState({ type: 'addEvents' })
  }

  const handleImportCsv = () => {
    setModalState({ type: 'csvImport' })
  }

  const handleImportEkasa = () => {
    setModalState({ type: 'ekasaImport' })
  }

  return (
    <section className="px-4 md:px-10 py-8">
      {/* Filter + action bar */}
      <div className="flex flex-wrap items-center gap-3 mb-4">
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{t('ledgerPage.filterFrom')}</span>
          <DatePicker value={fromDate || undefined} onChange={(d) => setFromDate(d)} clearable placeholder={t('ledgerPage.filterFrom')} />
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{t('ledgerPage.filterTo')}</span>
          <DatePicker value={toDate || undefined} onChange={(d) => setToDate(d)} clearable placeholder={t('ledgerPage.filterTo')} />
        </div>
        <PortfolioItemFilter accounts={snapshot} selectedIds={selectedAccountIds} onChange={setSelectedAccountIds} />
        <div className="ml-auto">
          <LedgerActionButtons onAddEvents={handleAddEvents} onImportCsv={handleImportCsv} onImportEkasa={handleImportEkasa} />
        </div>
      </div>

      {/* Event type filter */}
      <div className="flex items-center gap-1 mb-6">
        {(
          [
            { value: 'all', label: t('ledgerPage.filterType.all') },
            { value: 'balance_update', label: t('ledgerPage.filterType.balanceUpdates') },
            { value: 'cashflow', label: t('ledgerPage.filterType.cashflows') },
            { value: 'transfer', label: t('ledgerPage.filterType.transfers') },
            { value: 'revenue', label: t('ledgerPage.filterType.revenue') },
            { value: 'expense', label: t('ledgerPage.filterType.expense') },
            { value: 'prepaid_expense', label: t('ledgerPage.filterType.prepaid_expense') },
          ] as const
        ).map(({ value, label }) => (
          <Button
            key={value}
            variant={!unmatchedOnly && eventTypeFilter === value ? 'default' : 'outline'}
            size="sm"
            onClick={() => {
              setUnmatchedOnly(false)
              setEventTypeFilter(value)
            }}
          >
            {label}
          </Button>
        ))}
        <Button variant={unmatchedOnly ? 'default' : 'outline'} size="sm" onClick={() => setUnmatchedOnly(!unmatchedOnly)}>
          {t('ledger.filterUnmatched')} ({unmatchedCount})
        </Button>
      </div>

      {loading ? (
        <p className="text-sm text-muted-foreground">&hellip;</p>
      ) : (
        <LedgerEventList
          events={events}
          accounts={snapshot}
          consolidationCurrency={consolidationCurrency}
          onEditEvent={handleEditEvent}
          onDeleteEvent={handleDeleteEvent}
          onDeleteTransferEvent={handleDeleteTransferEvent}
          onDeleteSplitGroup={handleDeleteSplitGroup}
          onEditTaxableSplitGroup={handleEditTaxableSplitGroup}
        />
      )}
    </section>
  )
}

export default LedgerPage
