import { useQuery } from '@tanstack/react-query'
import { getAccountsSnapshot } from '../api'
import { toEndOfDay } from '../utils/format'

export const useSnapshotQuery = (selectedDate: string) => {
  return useQuery({
    queryKey: ['snapshot', selectedDate],
    queryFn: async () => {
      const result = await getAccountsSnapshot(toEndOfDay(selectedDate))
      return result
    },
  })
}
