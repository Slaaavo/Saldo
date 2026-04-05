import { useQuery } from '@tanstack/react-query'
import { getBulkUpdateExclusions } from '../api'

export function useBulkUpdateExclusionsQuery() {
  return useQuery({
    queryKey: ['bulk-update-exclusions'],
    queryFn: async () => {
      const result = await getBulkUpdateExclusions()
      return result
    },
  })
}
