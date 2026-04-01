import { useState, useCallback, useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EventWithData, SnapshotRow } from '../../shared/types'
import { listEvents } from '../../shared/api'
import { toEndOfDay } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'

interface UseLedgerDataOptions {
  refreshTrigger?: number
  snapshot?: SnapshotRow[]
}

export function useLedgerData({ refreshTrigger, snapshot }: UseLedgerDataOptions = {}) {
  const { t } = useTranslation()

  const [fromDate, setFromDate] = useState('')
  const [toDate, setToDate] = useState('')
  const [selectedAccountIds, setSelectedAccountIds] = useState<number[]>([])
  const [eventTypeFilter, setEventTypeFilter] = useState<string>('all')
  const [events, setEvents] = useState<EventWithData[]>([])
  const [loading, setLoading] = useState(false)

  const accountTypeMap = useMemo(() => {
    const map = new Map<number, string>()
    if (snapshot) {
      for (const row of snapshot) {
        map.set(row.accountId, row.accountType)
      }
    }
    return map
  }, [snapshot])

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const filter: Parameters<typeof listEvents>[0] = {}
      if (fromDate) filter.fromDate = `${fromDate}T00:00:00`
      if (toDate) filter.beforeDate = toEndOfDay(toDate)
      if (selectedAccountIds.length > 0) {
        const bucketFilterIds = selectedAccountIds.filter((id) => accountTypeMap.get(id) === 'bucket')
        // All selected IDs go to accountIds (buckets need it for their own balance_update events)
        if (selectedAccountIds.length > 0) filter.accountIds = selectedAccountIds
        // Bucket IDs additionally go to bucketIds (for cashflows tagged to the bucket)
        if (bucketFilterIds.length > 0) filter.bucketIds = bucketFilterIds
      }
      if (eventTypeFilter !== 'all') filter.eventTypes = [eventTypeFilter]
      const { events: evts } = await listEvents(filter)
      setEvents(evts)
    } catch (err) {
      toast.error(t('errors.loadData', { error: extractErrorMessage(err) }))
    } finally {
      setLoading(false)
    }
  }, [fromDate, toDate, selectedAccountIds, eventTypeFilter, accountTypeMap, t])

  // Re-fetch whenever filters change
  useEffect(() => {
    refresh()
  }, [refresh])

  // Re-fetch when an external refresh trigger fires (e.g. after modal actions)
  useEffect(() => {
    if (refreshTrigger !== undefined && refreshTrigger > 0) {
      refresh()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshTrigger])

  return {
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
    refresh,
  }
}
