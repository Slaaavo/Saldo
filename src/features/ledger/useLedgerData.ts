import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import type { EventWithData } from '../../shared/types';
import { listEvents } from '../../shared/api';
import { toEndOfDay } from '../../shared/utils/format';
import { extractErrorMessage } from '../../shared/utils/errors';

interface UseLedgerDataOptions {
  refreshTrigger?: number;
}

export function useLedgerData({ refreshTrigger }: UseLedgerDataOptions = {}) {
  const { t } = useTranslation();

  const [fromDate, setFromDate] = useState('');
  const [toDate, setToDate] = useState('');
  const [selectedAccountIds, setSelectedAccountIds] = useState<number[]>([]);
  const [events, setEvents] = useState<EventWithData[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const filter: Parameters<typeof listEvents>[0] = {};
      if (fromDate) filter.fromDate = `${fromDate}T00:00:00`;
      if (toDate) filter.beforeDate = toEndOfDay(toDate);
      if (selectedAccountIds.length > 0) filter.accountIds = selectedAccountIds;
      const { events: evts } = await listEvents(filter);
      setEvents(evts);
    } catch (err) {
      toast.error(t('errors.loadData', { error: extractErrorMessage(err) }));
    } finally {
      setLoading(false);
    }
  }, [fromDate, toDate, selectedAccountIds, t]);

  // Re-fetch whenever filters change
  useEffect(() => {
    refresh();
  }, [refresh]);

  // Re-fetch when an external refresh trigger fires (e.g. after modal actions)
  useEffect(() => {
    if (refreshTrigger !== undefined && refreshTrigger > 0) {
      refresh();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshTrigger]);

  return {
    fromDate,
    setFromDate,
    toDate,
    setToDate,
    selectedAccountIds,
    setSelectedAccountIds,
    events,
    loading,
    refresh,
  };
}
