import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import type { SnapshotRow } from '../../shared/types'
import { listEvents } from '../../shared/api'
import { toEndOfDay } from '../../shared/utils/format'
import { useSelectedPerson } from '../../app/useSelectedPerson'

interface UseLedgerDataOptions {
  snapshot?: SnapshotRow[]
  /** The account the page opens filtered to, when the user came from that account's card. */
  initialAccountId?: number
}

export const useLedgerData = ({ snapshot, initialAccountId }: UseLedgerDataOptions = {}) => {
  const [fromDate, setFromDate] = useState('')
  const [toDate, setToDate] = useState('')
  const [selectedAccountIds, setSelectedAccountIds] = useState<number[]>(initialAccountId === undefined ? [] : [initialAccountId])
  const [eventTypeFilter, setEventTypeFilter] = useState<string>('all')
  const [unmatchedOnly, setUnmatchedOnly] = useState(false)
  const { selectedPersonId } = useSelectedPerson()

  const [prevPersonId, setPrevPersonId] = useState(selectedPersonId)
  if (prevPersonId !== selectedPersonId) {
    setPrevPersonId(selectedPersonId)
    setSelectedAccountIds([])
  }

  const bucketFilterIds = useMemo(() => {
    const bucketIds = new Set(snapshot?.filter((row) => row.accountType === 'bucket').map((row) => row.accountId))
    return selectedAccountIds.filter((id) => bucketIds.has(id))
  }, [snapshot, selectedAccountIds])

  const { data, isPending } = useQuery({
    queryKey: ['events', 'ledger', fromDate, toDate, selectedAccountIds, bucketFilterIds, eventTypeFilter, unmatchedOnly, selectedPersonId],
    queryFn: async () => {
      const filter: Parameters<typeof listEvents>[0] = {}
      if (fromDate) filter.fromDate = `${fromDate}T00:00:00`
      if (toDate) filter.beforeDate = toEndOfDay(toDate)
      if (selectedAccountIds.length > 0) {
        // All selected IDs go to accountIds (buckets need it for their own balance_update events)
        filter.accountIds = selectedAccountIds
        // Bucket IDs additionally go to bucketIds (for cashflows tagged to the bucket)
        if (bucketFilterIds.length > 0) filter.bucketIds = bucketFilterIds
      }
      if (unmatchedOnly) {
        filter.unmatchedOnly = true
      } else if (eventTypeFilter !== 'all') {
        filter.eventTypes = [eventTypeFilter]
      }
      if (selectedPersonId !== null) filter.personId = selectedPersonId
      const { events: evts } = await listEvents(filter)
      return evts
    },
  })

  const hasActiveFilters = fromDate !== '' || toDate !== '' || selectedAccountIds.length > 0 || eventTypeFilter !== 'all' || unmatchedOnly

  const clearFilters = () => {
    setFromDate('')
    setToDate('')
    setSelectedAccountIds([])
    setEventTypeFilter('all')
    setUnmatchedOnly(false)
  }

  return {
    hasActiveFilters,
    clearFilters,
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
    events: data ?? [],
    loading: isPending,
  }
}
