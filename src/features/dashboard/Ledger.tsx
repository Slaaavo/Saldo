import { useTranslation } from 'react-i18next';
import type { EventWithData, SnapshotRow, Currency } from '../../shared/types';
import LedgerEventList from '../../shared/ui/LedgerEventList';
import { Button } from '../../shared/ui/button';
import { RefreshCw } from 'lucide-react';

interface Props {
  events: EventWithData[];
  accounts: SnapshotRow[];
  consolidationCurrency?: Currency | null;
  onEditEvent: (event: EventWithData) => void;
  onDeleteEvent: (eventId: number) => void;
  onUpdateBalances: () => void;
  totalEvents?: number;
  onViewAll?: () => void;
}

export default function Ledger({
  events,
  accounts,
  consolidationCurrency,
  onEditEvent,
  onDeleteEvent,
  onUpdateBalances,
  totalEvents,
  onViewAll,
}: Props) {
  const { t } = useTranslation();

  const showViewAll =
    totalEvents !== undefined && events.length < totalEvents && onViewAll !== undefined;

  return (
    <section>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold">{t('ledger.title')}</h2>
        <Button onClick={onUpdateBalances} size="sm">
          <RefreshCw className="h-4 w-4" />
          {t('ledger.updateBalances')}
        </Button>
      </div>

      <LedgerEventList
        events={events}
        accounts={accounts}
        consolidationCurrency={consolidationCurrency}
        onEditEvent={onEditEvent}
        onDeleteEvent={onDeleteEvent}
      />

      {showViewAll && (
        <div className="mt-4 text-center">
          <button
            onClick={onViewAll}
            className="text-sm text-muted-foreground hover:text-foreground underline underline-offset-2 transition-colors"
          >
            {t('ledger.viewAll', { shown: events.length, total: totalEvents })}
          </button>
        </div>
      )}
    </section>
  );
}
