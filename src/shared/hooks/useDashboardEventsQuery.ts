import { useQuery } from '@tanstack/react-query'
import { listEvents } from '../api'
import { toEndOfDay } from '../utils/format'

export const useDashboardEventsQuery = (selectedDate: string) => {
  return useQuery({
    queryKey: ['events', 'dashboard', selectedDate],
    queryFn: async () => {
      const result = await listEvents({ beforeDate: toEndOfDay(selectedDate), limit: 20 })
      return result
    },
  })
}
