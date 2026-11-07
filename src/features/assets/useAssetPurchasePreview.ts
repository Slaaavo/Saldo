import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { previewAssetPurchase } from '../../shared/api'
import { pctToBps } from '../../shared/utils/format'
import { purchaseVatFromDraft, type AssetUseDraft } from './assetUseDraft'

interface PreviewParams {
  draft: AssetUseDraft
  minorUnits: number
  showVatFields: boolean
  purchaseDate: string
  depreciationPeriod: string
}

export const useAssetPurchasePreview = ({ draft, minorUnits, showVatFields, purchaseDate, depreciationPeriod }: PreviewParams) => {
  const purchaseVat = purchaseVatFromDraft(draft, minorUnits, showVatFields)
  const businessUsePctBps = pctToBps(draft.businessUsePct)
  const months = parseInt(depreciationPeriod, 10)
  const input = {
    purchaseVat,
    businessUsePctBps,
    purchaseDate: purchaseDate || null,
    depreciationPeriodMonths: months > 0 ? months : null,
  }

  const { data } = useQuery({
    queryKey: ['assetPurchasePreview', input],
    queryFn: () => previewAssetPurchase({ ...input, purchaseVat: purchaseVat!, businessUsePctBps: businessUsePctBps! }),
    enabled: purchaseVat !== null && businessUsePctBps !== null,
    placeholderData: keepPreviousData,
  })

  return purchaseVat === null ? undefined : data
}
