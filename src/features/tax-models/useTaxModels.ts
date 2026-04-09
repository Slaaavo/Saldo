import { useQuery, useQueryClient } from '@tanstack/react-query'
import { listTaxModels, deleteTaxModel } from '../../shared/api'
import type { TaxModelRow } from '../../shared/types'

export const useTaxModels = () => {
  const queryClient = useQueryClient()
  const { data: taxModels = [], isLoading } = useQuery<TaxModelRow[]>({ queryKey: ['taxModels'], queryFn: listTaxModels })

  const handleDeleteTaxModel = async (modelId: number) => {
    await deleteTaxModel(modelId)
    await queryClient.invalidateQueries({ queryKey: ['taxModels'] })
  }

  return {
    taxModels,
    isLoading,
    handleDeleteTaxModel,
  }
}
