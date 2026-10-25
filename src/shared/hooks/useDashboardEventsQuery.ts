import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { listEvents } from '../api'
import type { AsOf } from '../types'
import { toEndOfDay } from '../utils/format'
import { useSelectedPerson } from '../../app/useSelectedPerson'

export const useDashboardEventsQuery = (asOf: AsOf) => {
  const { selectedPersonId } = useSelectedPerson()
  return useQuery({
    queryKey: ['events', 'dashboard', asOf, selectedPersonId],
    queryFn: () =>
      listEvents({
        beforeDate: asOf.kind === 'at' ? toEndOfDay(asOf.until.substring(0, 10)) : undefined,
        limit: 20,
        personId: selectedPersonId ?? undefined,
      }),
    placeholderData: keepPreviousData,
  })
}
