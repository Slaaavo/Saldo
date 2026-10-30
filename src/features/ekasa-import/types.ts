import type { SplitLegDraft } from '../transactions/splitLegDraft'

export type EkasaWizardStep = 'upload' | 'rules' | 'review' | 'save-profile' | 'error'

export interface EkasaRuleDraft {
  id: string
  namePattern: string
  deductiblePct: string
  vatReclaimablePct: string
}

export interface ReceiptReadLine {
  name: string
  grossMinor: number
  vatRateBps: number
}

// An offline receipt's QR code holds only its date and total, so it has no shop and no lines.
export type ProcessReceiptResult = {
  date: string
  totalMinor: number
  sourceRef: string
  alreadyImported: boolean
} & ({ kind: 'online'; vendor: string; lines: ReceiptReadLine[] } | { kind: 'offline'; vendor: null; lines: null })

export interface MatchEkasaRulesPayload {
  itemNames: string[]
  // The rules step's patterns in row order, blank rows included.
  patterns: string[]
}

export interface EkasaRuleMatches {
  // Per item name, the row of the first rule that matches it, numbered from 1 as in invalidRules.
  matchedRows: Array<number | null>
  invalidRules: Array<{ position: number; pattern: string; message: string }>
}

export interface EkasaWizardState {
  step: EkasaWizardStep
  filePath: string | null
  processedLegs: SplitLegDraft[]
  receiptError: string | null
  rules: EkasaRuleDraft[]
  defaultDeductiblePct: string
  defaultVatReclaimablePct: string
  isProcessing: boolean
  vendorName: string
  receiptDate: string
  sourceRef: string | null
  alreadyImported: boolean
}
