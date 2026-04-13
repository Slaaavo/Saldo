import { useTranslation } from 'react-i18next'
import type { EventWithData, SnapshotRow, Currency } from '../../shared/types'
import LedgerEventList from '../../shared/ui/LedgerEventList'
import LedgerActionButtons from '../../shared/ui/LedgerActionButtons'

interface Props {
  events: EventWithData[]
  accounts: SnapshotRow[]
  consolidationCurrency?: Currency | null
  onEditEvent: (event: EventWithData) => void
  onDeleteEvent: (eventId: number, eventType?: string) => void
  onDeleteTransferEvent?: (eventId: number, linkedEventId: number) => void
  onDeleteSplitGroup?: (splitGroupId: number) => void
  onAddEvents: () => void
  totalEvents?: number
  onViewAll?: () => void
  onImportCsv: () => void
  onImportEkasa: () => void
  onEditTaxableSplitGroup?: (splitGroupId: number, eventType: string, legs: EventWithData[], groupNote: string | null, accountId: number) => void
}

const Ledger = ({
  events,
  accounts,
  consolidationCurrency,
  onEditEvent,
  onDeleteEvent,
  onDeleteTransferEvent,
  onDeleteSplitGroup,
  onAddEvents,
  totalEvents,
  onViewAll,
  onImportCsv,
  onImportEkasa,
  onEditTaxableSplitGroup,
}: Props) => {
  const { t } = useTranslation()

  const showViewAll = totalEvents !== undefined && events.length < totalEvents && onViewAll !== undefined

  return (
    <section>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold">{t('ledger.title')}</h2>
        <LedgerActionButtons onAddEvents={onAddEvents} onImportCsv={onImportCsv} onImportEkasa={onImportEkasa} />
      </div>

      <LedgerEventList
        events={events}
        accounts={accounts}
        consolidationCurrency={consolidationCurrency}
        onEditEvent={onEditEvent}
        onDeleteEvent={onDeleteEvent}
        onDeleteTransferEvent={onDeleteTransferEvent}
        onDeleteSplitGroup={onDeleteSplitGroup}
        onEditTaxableSplitGroup={onEditTaxableSplitGroup}
      />

      {showViewAll && (
        <div className="mt-4 text-center">
          <button onClick={onViewAll} className="text-sm text-muted-foreground hover:text-foreground underline underline-offset-2 transition-colors">
            {t('ledger.viewAll', { shown: events.length, total: totalEvents })}
          </button>
        </div>
      )}
    </section>
  )
}

export default Ledger
