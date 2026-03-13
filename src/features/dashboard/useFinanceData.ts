import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { EventWithData, SnapshotRow, Currency } from '../../shared/types';
import { getAccountsSnapshot, listEvents, getConsolidationCurrency } from '../../shared/api';
import { toEndOfDay, todayIso } from '../../shared/utils/format';
import { extractErrorMessage } from '../../shared/utils/errors';

const DASHBOARD_LEDGER_LIMIT = 20;

export function useFinanceData() {
  const { t } = useTranslation();

  const [selectedDate, setSelectedDate] = useState(todayIso());
  const [snapshot, setSnapshot] = useState<SnapshotRow[]>([]);
  const [events, setEvents] = useState<EventWithData[]>([]);
  const [totalEvents, setTotalEvents] = useState<number>(0);
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null);

  useEffect(() => {
    getConsolidationCurrency()
      .then(setConsolidationCurrency)
      .catch((err) => console.error('Failed to load consolidation currency:', err));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const endOfDay = toEndOfDay(selectedDate);
      const [snap, { events: evts, totalCount }] = await Promise.all([
        getAccountsSnapshot(endOfDay),
        listEvents({ beforeDate: endOfDay, limit: DASHBOARD_LEDGER_LIMIT }),
      ]);
      setSnapshot(snap);
      setEvents(evts);
      setTotalEvents(totalCount);
    } catch (err) {
      toast.error(t('errors.loadData', { error: extractErrorMessage(err) }));
    }
  }, [selectedDate, t]);

  useEffect(() => {
    const load = async () => {
      await refresh();
    };
    load();
  }, [refresh]);

  const handleConsolidationCurrencyChange = useCallback(async () => {
    try {
      const currency = await getConsolidationCurrency();
      setConsolidationCurrency(currency);
      await refresh();
    } catch (err) {
      console.error('Failed to reload after currency change:', err);
    }
  }, [refresh]);

  const missingFxCurrencies = [
    ...new Set(snapshot.filter((r) => r.fxRateMissing).map((r) => r.currencyCode)),
  ];

  return {
    selectedDate,
    setSelectedDate,
    snapshot,
    events,
    totalEvents,
    consolidationCurrency,
    refresh,
    handleConsolidationCurrencyChange,
    missingFxCurrencies,
  };
}
