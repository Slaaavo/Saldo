import { useQuery } from '@tanstack/react-query'
import { getConsolidationCurrency } from '../api'

export function useConsolidationCurrencyQuery() {
  return useQuery({
    queryKey: ['consolidation-currency'],
    queryFn: async () => {
      const result = await getConsolidationCurrency()
      return result
    },
  })
}
