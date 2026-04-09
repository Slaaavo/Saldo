import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQuery } from '@tanstack/react-query'
import type { EventWithData } from '../../shared/types'
import { getEventById, getUnmatchedCashflowCount } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import { useLedgerData } from './useLedgerData'
import LedgerEventList from '../../shared/ui/LedgerEventList'
import PortfolioItemFilter from './PortfolioItemFilter'
import { DatePicker } from '../../shared/ui/date-picker'
import { Button } from '../../shared/ui/button'
import { Plus, Upload } from 'lucide-react'
import { useSnapshotQuery } from '../../shared/hooks/useSnapshotQuery'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useModal } from '../../app/useModal'
import { todayIso } from '../../shared/utils/format'
import { useSelectedPerson } from '../../app/useSelectedPerson'

const LedgerPage = () => {
  const { t } = useTranslation()
  const { setModalState } = useModal()
  const { selectedPersonId } = useSelectedPerson()
  const snapshotQuery = useSnapshotQuery(todayIso())
  const snapshot = snapshotQuery.data ?? []
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const consolidationCurrency = consolidationCurrencyQuery.data ?? null

  const unmatchedCountQuery = useQuery({
    queryKey: ['unmatched-cashflow-count', selectedPersonId],
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

  const handleEditEvent = async (event: EventWithData) => {
    if (event.eventType === 'balance_update') {
      setModalState({ type: 'editBalanceUpdate', event })
      return
    }
    if (event.eventType === 'revenue' || event.eventType === 'expense') {
      if (event.splitGroupId != null) {
        const groupId = event.splitGroupId
        const legs = events.filter((e) => e.splitGroupId === groupId)
        const groupNote = event.splitGroupNote ?? null
        setModalState({
          type: 'editTaxableSplitGroup',
          splitGroupId: groupId,
          eventType: event.eventType as 'revenue' | 'expense',
          legs,
          groupNote,
          accountId: event.accountId,
        })
      } else {
        setModalState({ type: 'editTaxableEvent', event })
      }
      return
    }
    if (event.eventType === 'transfer') {
      if (!event.linkedEventId) {
        toast.error(t('errors.loadData'))
        return
      }
      try {
        const linked = await getEventById(event.linkedEventId)
        if (!linked) {
          toast.error(t('errors.loadData'))
          return
        }
        const fromEvent = event.amountMinor < 0 ? event : linked
        const toEvent = event.amountMinor < 0 ? linked : event
        setModalState({ type: 'editTransfer', fromEvent, toEvent })
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    }
  }

  const handleEditTaxableSplitGroup = (splitGroupId: number, eventType: string, legs: EventWithData[], groupNote: string | null, accountId: number) => {
    setModalState({ type: 'editTaxableSplitGroup', splitGroupId, eventType: eventType as 'revenue' | 'expense', legs, groupNote, accountId })
  }

  const handleDeleteEvent = (eventId: number) => {
    setModalState({ type: 'confirmDeleteEvent', eventId })
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
        <div className="ml-auto flex items-center gap-2">
          <Button onClick={handleImportCsv} size="sm" variant="outline">
            <Upload className="h-4 w-4" />
            {t('import.title')}
          </Button>
          <Button onClick={handleAddEvents} size="sm">
            <Plus className="h-4 w-4" />
            {t('ledger.addEvents')}
          </Button>
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
