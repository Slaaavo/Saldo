import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import type { SnapshotRow } from '../../shared/types'
import { listEvents } from '../../shared/api'
import { toEndOfDay } from '../../shared/utils/format'

interface UseLedgerDataOptions {
  snapshot?: SnapshotRow[]
}

export function useLedgerData({ snapshot }: UseLedgerDataOptions = {}) {
  const [fromDate, setFromDate] = useState('')
  const [toDate, setToDate] = useState('')
  const [selectedAccountIds, setSelectedAccountIds] = useState<number[]>([])
  const [eventTypeFilter, setEventTypeFilter] = useState<string>('all')

  const accountTypeMap = useMemo(() => {
    const map = new Map<number, string>()
    if (snapshot) {
      for (const row of snapshot) {
        map.set(row.accountId, row.accountType)
      }
    }
    return map
  }, [snapshot])

  const { data, isPending, isFetching } = useQuery({
    queryKey: ['events', 'ledger', fromDate, toDate, selectedAccountIds, eventTypeFilter],
    queryFn: async () => {
      const filter: Parameters<typeof listEvents>[0] = {}
      if (fromDate) filter.fromDate = `${fromDate}T00:00:00`
      if (toDate) filter.beforeDate = toEndOfDay(toDate)
      if (selectedAccountIds.length > 0) {
        const bucketFilterIds = selectedAccountIds.filter((id) => accountTypeMap.get(id) === 'bucket')
        // All selected IDs go to accountIds (buckets need it for their own balance_update events)
        filter.accountIds = selectedAccountIds
        // Bucket IDs additionally go to bucketIds (for cashflows tagged to the bucket)
        if (bucketFilterIds.length > 0) filter.bucketIds = bucketFilterIds
      }
      if (eventTypeFilter !== 'all') filter.eventTypes = [eventTypeFilter]
      const { events: evts } = await listEvents(filter)
      return evts
    },
  })

  return {
    fromDate,
    setFromDate,
    toDate,
    setToDate,
    selectedAccountIds,
    setSelectedAccountIds,
    eventTypeFilter,
    setEventTypeFilter,
    events: data ?? [],
    loading: isPending || isFetching,
  }
}
