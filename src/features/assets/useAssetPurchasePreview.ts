import { keepPreviousData, skipToken, useQuery } from '@tanstack/react-query'
import { previewAssetPurchase } from '../../shared/api'
import { pctToBps } from '../../shared/utils/format'
import { FULL_BUSINESS_USE_BPS, purchaseVatFromDraft, type AssetUseDraft } from './assetUseDraft'

interface PreviewParams {
  draft: AssetUseDraft
  minorUnits: number
  showVatFields: boolean
  depreciationPeriod: string
}

export const useAssetPurchasePreview = ({ draft, minorUnits, showVatFields, depreciationPeriod }: PreviewParams) => {
  const purchaseVat = purchaseVatFromDraft(draft, minorUnits, showVatFields)
  const months = parseInt(depreciationPeriod, 10)
  const input = {
    businessUsePctBps: pctToBps(draft.businessUsePct) ?? FULL_BUSINESS_USE_BPS,
    depreciationPeriodMonths: months > 0 ? months : null,
  }

  const { data } = useQuery({
    queryKey: ['assetPurchasePreview', purchaseVat, input],
    queryFn: purchaseVat ? () => previewAssetPurchase({ ...input, purchaseVat }) : skipToken,
    placeholderData: keepPreviousData,
  })

  return purchaseVat === null ? undefined : data
}
