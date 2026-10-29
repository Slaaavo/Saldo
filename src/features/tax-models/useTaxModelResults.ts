import { useQuery } from '@tanstack/react-query'
import { calculateTaxModel } from '@/shared/api'

export const useTaxModelResults = (modelId: number, enabled = true) =>
  useQuery({
    queryKey: ['taxModelResults', modelId],
    queryFn: () => calculateTaxModel(modelId),
    enabled,
    staleTime: 0,
  })
