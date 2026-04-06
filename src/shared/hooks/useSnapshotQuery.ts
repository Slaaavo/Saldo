import { useQuery } from '@tanstack/react-query'
import { getAccountsSnapshot } from '../api'
import { toEndOfDay } from '../utils/format'
import { useSelectedPerson } from '../../app/useSelectedPerson'

export const useSnapshotQuery = (selectedDate: string) => {
  const { selectedPersonId } = useSelectedPerson()
  return useQuery({
    queryKey: ['snapshot', selectedDate, selectedPersonId],
    queryFn: async () => {
      const result = await getAccountsSnapshot(toEndOfDay(selectedDate), selectedPersonId ?? undefined)
      return result
    },
  })
}
