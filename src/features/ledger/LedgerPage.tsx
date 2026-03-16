import { useTranslation } from 'react-i18next';
import type { SnapshotRow, Currency, ModalState, EventWithData } from '../../shared/types';
import { useLedgerData } from './useLedgerData';
import LedgerEventList from '../../shared/ui/LedgerEventList';
import PortfolioItemFilter from './PortfolioItemFilter';
import { DatePicker } from '../../shared/ui/date-picker';
import { Button } from '../../shared/ui/button';
import { RefreshCw, Upload } from 'lucide-react';

interface Props {
  snapshot: SnapshotRow[];
  consolidationCurrency: Currency | null;
  setModalState: (state: ModalState) => void;
  refreshTrigger: number;
}

export default function LedgerPage({
  snapshot,
  consolidationCurrency,
  setModalState,
  refreshTrigger,
}: Props) {
  const { t } = useTranslation();

  const {
    fromDate,
    setFromDate,
    toDate,
    setToDate,
    selectedAccountIds,
    setSelectedAccountIds,
    eventTypeFilter,
    setEventTypeFilter,
    events,
    loading,
  } = useLedgerData({ refreshTrigger });

  const handleEditEvent = (event: EventWithData) => {
    setModalState({ type: 'editBalanceUpdate', event });
  };

  const handleDeleteEvent = (eventId: number) => {
    setModalState({ type: 'confirmDeleteEvent', eventId });
  };

  const handleDeleteTransferEvent = (eventId: number, linkedEventId: number) => {
    setModalState({ type: 'confirmDeleteTransferEvent', eventId, linkedEventId });
  };

  const handleUpdateBalances = () => {
    setModalState({ type: 'bulkUpdateBalance' });
  };

  const handleImportCsv = () => {
    setModalState({ type: 'csvImport' });
  };

  return (
    <section className="px-4 md:px-10 py-8">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-xl font-semibold">{t('ledgerPage.title')}</h2>
        <div className="flex items-center gap-2">
          <Button onClick={handleImportCsv} size="sm" variant="outline">
            <Upload className="h-4 w-4" />
            {t('import.title')}
          </Button>
          <Button onClick={handleUpdateBalances} size="sm">
            <RefreshCw className="h-4 w-4" />
            {t('ledger.updateBalances')}
          </Button>
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex flex-wrap items-center gap-3 mb-4">
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{t('ledgerPage.filterFrom')}</span>
          <DatePicker
            value={fromDate || undefined}
            onChange={(d) => setFromDate(d)}
            clearable
            placeholder={t('ledgerPage.filterFrom')}
          />
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{t('ledgerPage.filterTo')}</span>
          <DatePicker
            value={toDate || undefined}
            onChange={(d) => setToDate(d)}
            clearable
            placeholder={t('ledgerPage.filterTo')}
          />
        </div>
        <PortfolioItemFilter
          accounts={snapshot}
          selectedIds={selectedAccountIds}
          onChange={setSelectedAccountIds}
        />
      </div>

      {/* Event type filter */}
      <div className="flex items-center gap-1 mb-6">
        {(
          [
            { value: 'all', label: t('ledgerPage.filterType.all') },
            { value: 'balance_update', label: t('ledgerPage.filterType.balanceUpdates') },
            { value: 'cashflow', label: t('ledgerPage.filterType.cashflows') },
            { value: 'transfer', label: t('ledgerPage.filterType.transfers') },
          ] as const
        ).map(({ value, label }) => (
          <Button
            key={value}
            variant={eventTypeFilter === value ? 'default' : 'outline'}
            size="sm"
            onClick={() => setEventTypeFilter(value)}
          >
            {label}
          </Button>
        ))}
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
        />
      )}
    </section>
  );
}
