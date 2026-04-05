import { useState, useCallback, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EventWithData, SnapshotRow, Currency } from '../../shared/types'
import { getAccountsSnapshot, listEvents, getConsolidationCurrency } from '../../shared/api'
import { toEndOfDay, todayIso } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'

// Note 1: Keep display-related limits as constants so paging behavior is explicit and easy to tune.
const DASHBOARD_LEDGER_LIMIT = 20

export function useFinanceData() {
  // Note 2: Translation function is read once and then used in error messages so UI text remains locale-aware.
  const { t } = useTranslation()

  // Note 3: This state stores a plain date string (not Date object) because the API layer expects ISO-like input.
  const [selectedDate, setSelectedDate] = useState(todayIso())
  // Note 4: Snapshot is the account balance view at the chosen date.
  const [snapshot, setSnapshot] = useState<SnapshotRow[]>([])
  // Note 5: Events hold recent ledger rows shown on the dashboard.
  const [events, setEvents] = useState<EventWithData[]>([])
  // Note 6: Total count can be larger than the loaded page size, so keep it separate from events.length.
  const [totalEvents, setTotalEvents] = useState<number>(0)
  // Note 7: Null means "not loaded yet" and avoids pretending a currency exists before the API returns.
  const [consolidationCurrency, setConsolidationCurrency] = useState<Currency | null>(null)

  useEffect(() => {
    // Note 8: This effect runs once on mount to load app-level currency context used by the dashboard totals.
    const loadConsolidationCurrency = async () => {
      try {
        const currency = await getConsolidationCurrency()
        setConsolidationCurrency(currency)
      } catch (err) {
        console.error('Failed to load consolidation currency:', err)
      }
    }

    loadConsolidationCurrency()
  }, [])

  const refresh = useCallback(async () => {
    try {
      // Note 9: Convert selected day to end-of-day to include all events that happened on that calendar day.
      const endOfDay = toEndOfDay(selectedDate)
      // Note 10: Parallel requests reduce dashboard wait time because snapshot and ledger are independent reads.
      const [snapshot, { events: events, totalCount }] = await Promise.all([getAccountsSnapshot(endOfDay), listEvents({ beforeDate: endOfDay, limit: DASHBOARD_LEDGER_LIMIT })])
      setSnapshot(snapshot)
      setEvents(events)
      setTotalEvents(totalCount)
    } catch (err) {
      // Note 11: User-facing errors should be localized and sanitized rather than exposing raw exception objects.
      toast.error(t('errors.loadData', { error: extractErrorMessage(err) }))
    }
  }, [selectedDate, t])

  useEffect(() => {
    // Note 12: Wrap the async call in a local function because effect callbacks themselves cannot be async.
    const load = async () => {
      await refresh()
    }
    // Note 13: Re-run whenever the memoized refresh function changes (for example when selectedDate changes).
    load()
  }, [refresh])

  const handleConsolidationCurrencyChange = useCallback(async () => {
    try {
      // Note 14: Re-read currency from source of truth, then refresh dependent dashboard data.
      const currency = await getConsolidationCurrency()
      setConsolidationCurrency(currency)
      await refresh()
    } catch (err) {
      // Note 15: This path logs diagnostic info for developers; user-facing failures are handled in refresh.
      console.error('Failed to reload after currency change:', err)
    }
  }, [refresh])

  // Note 16: Build a unique list of currencies that currently miss FX rates so the UI can show a concise warning.
  const missingFxCurrencies = [...new Set(snapshot.filter((r) => r.fxRateMissing).map((r) => r.currencyCode))]

  // Note 17: Returning both data and mutation handlers makes this hook a focused view-model for the dashboard page.
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
  }
}
