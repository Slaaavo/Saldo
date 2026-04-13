import type { SplitLegDraft } from '../transactions/splitLegDraft'

export type EkasaWizardStep = 'upload' | 'rules' | 'review' | 'save-profile' | 'error'

export interface EkasaRuleDraft {
  id: string
  namePattern: string
  deductiblePct: string
  vatReclaimablePct: string
}

export interface EkasaProcessedItem {
  name: string
  amountMinor: number
  vatRateBps: number
}

export interface EkasaReceiptOrganization {
  name: string
  ico: string
}

export interface EkasaReceiptData {
  id: string
  receiptId: string
  issueDate: string
  totalPrice: number
  organization: EkasaReceiptOrganization
  items: Array<{ name: string; quantity: number; price: number; vatRate: string }>
  vatSummary: Array<{ vatRate: string; vatBase: number; vatAmount: number }>
}

export interface OfflineReceiptData {
  eventDate: string
  totalAmountMinor: number
}

export interface ProcessReceiptResult {
  receiptData: EkasaReceiptData | null
  processedItems: EkasaProcessedItem[]
  qrContent: string
  offlineFallback: OfflineReceiptData | null
}

export interface EkasaWizardState {
  step: EkasaWizardStep
  filePath: string | null
  receiptData: EkasaReceiptData | null
  processedItems: EkasaProcessedItem[]
  processedLegs: SplitLegDraft[]
  receiptError: string | null
  rules: EkasaRuleDraft[]
  defaultDeductiblePct: string
  defaultVatReclaimablePct: string
  isProcessing: boolean
  vendorName: string
  receiptDate: string
  offlineFallback: OfflineReceiptData | null
}
