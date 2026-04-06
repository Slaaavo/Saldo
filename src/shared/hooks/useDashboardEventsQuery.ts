import { useQuery } from '@tanstack/react-query'
import { listEvents } from '../api'
import { toEndOfDay } from '../utils/format'
import { useSelectedPerson } from '../../app/useSelectedPerson'

export const useDashboardEventsQuery = (selectedDate: string) => {
  const { selectedPersonId } = useSelectedPerson()
  return useQuery({
    queryKey: ['events', 'dashboard', selectedDate, selectedPersonId],
    queryFn: async () => {
      const result = await listEvents({ beforeDate: toEndOfDay(selectedDate), limit: 20, personId: selectedPersonId ?? undefined })
      return result
    },
  })
}
