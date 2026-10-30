import { useState, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EkasaWizardState, EkasaRuleDraft, EkasaProcessedItem, EkasaRuleMatches, MatchEkasaRulesPayload, OfflineReceiptData, ProcessReceiptResult } from './types'
import { makeEmptyLeg } from '../transactions/splitLegDraft'
import type { SplitLegDraft } from '../transactions/splitLegDraft'
import { fromMinorUnits, bpsToPct, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { getEkasaProfile, upsertEkasaProfile, processReceiptFile, matchEkasaRules } from '../../shared/api'

const INITIAL_STATE: EkasaWizardState = {
  step: 'upload',
  filePath: null,
  receiptData: null,
  processedItems: [],
  processedLegs: [],
  receiptError: null,
  rules: [],
  defaultDeductiblePct: '100',
  defaultVatReclaimablePct: '100',
  isProcessing: false,
  vendorName: '',
  receiptDate: '',
  offlineFallback: null,
  sourceRef: null,
  alreadyImported: false,
}

const matchPayload = (items: EkasaProcessedItem[], rules: EkasaRuleDraft[]): MatchEkasaRulesPayload => ({
  itemNames: items.map((item) => item.name),
  rules: rules.map((rule) => ({
    namePattern: rule.namePattern,
    deductiblePctBps: pctToBps(rule.deductiblePct) ?? 0,
    vatReclaimablePctBps: pctToBps(rule.vatReclaimablePct) ?? 0,
  })),
})

const legsFromMatches = (items: EkasaProcessedItem[], matches: EkasaRuleMatches, defaultDeductiblePct: string, defaultVatReclaimablePct: string, date: string): SplitLegDraft[] =>
  items.map((item, index) => {
    const rule = matches.items[index]?.matchedRule ?? null
    return {
      ...makeEmptyLeg(date),
      amount: fromMinorUnits(item.amountMinor, 2),
      vatRate: bpsToPct(item.vatRateBps),
      note: item.name,
      expenseDeductiblePct: rule ? bpsToPct(rule.deductiblePctBps) : defaultDeductiblePct,
      vatReclaimablePct: rule ? bpsToPct(rule.vatReclaimablePctBps) : defaultVatReclaimablePct,
    }
  })

interface ProfileSnapshot {
  rules: Array<{ namePattern: string; deductiblePct: string; vatReclaimablePct: string }>
  defaultDeductiblePct: string
  defaultVatReclaimablePct: string
}

// Offline receipts carry no item detail, so the review step gets a single leg for the total.
const reviewOfflineReceipt = (prev: EkasaWizardState, result: ProcessReceiptResult, offline: OfflineReceiptData): EkasaWizardState => {
  const leg: SplitLegDraft = {
    ...makeEmptyLeg(offline.eventDate),
    amount: fromMinorUnits(offline.totalAmountMinor, 2),
    expenseDeductiblePct: prev.defaultDeductiblePct,
    vatReclaimablePct: prev.defaultVatReclaimablePct,
  }
  return {
    ...prev,
    receiptData: null,
    processedItems: [],
    processedLegs: [leg],
    receiptDate: offline.eventDate,
    vendorName: '',
    offlineFallback: offline,
    sourceRef: result.sourceRef,
    alreadyImported: result.alreadyImported,
    isProcessing: false,
    step: 'review',
  }
}

const snapshotFromState = (s: EkasaWizardState): ProfileSnapshot => ({
  rules: s.rules.map((r) => ({ namePattern: r.namePattern, deductiblePct: r.deductiblePct, vatReclaimablePct: r.vatReclaimablePct })),
  defaultDeductiblePct: s.defaultDeductiblePct,
  defaultVatReclaimablePct: s.defaultVatReclaimablePct,
})

export const useEkasaImportWizard = () => {
  const { t } = useTranslation()
  const [state, setState] = useState<EkasaWizardState>(INITIAL_STATE)
  const processingRef = useRef<Promise<ProcessReceiptResult> | null>(null)
  const originalProfileRef = useRef<ProfileSnapshot>(snapshotFromState(INITIAL_STATE))

  const handleFileSelect = useCallback((filePath: string) => {
    const promise = processReceiptFile(filePath)
    processingRef.current = promise
    setState((prev) => ({
      ...prev,
      filePath,
      isProcessing: true,
      offlineFallback: null,
      step: 'rules',
    }))
    promise
      .then(() => {
        if (processingRef.current !== promise) return
        setState((prev) => ({ ...prev, isProcessing: false }))
      })
      .catch((err) => {
        if (processingRef.current !== promise) return
        setState((prev) => ({
          ...prev,
          receiptError: extractErrorMessage(err),
          isProcessing: false,
          step: 'error',
        }))
      })
  }, [])

  const handleRulesConfirm = useCallback(async () => {
    const promise = processingRef.current
    let result: ProcessReceiptResult
    try {
      if (!promise) throw new Error('No file selected')
      result = await promise
    } catch (err) {
      if (processingRef.current !== promise) return
      setState((prev) => ({ ...prev, receiptError: extractErrorMessage(err), isProcessing: false, step: 'error' }))
      return
    }
    if (processingRef.current !== promise) return

    if (result.offlineFallback && result.processedItems.length === 0) {
      const offline = result.offlineFallback
      setState((prev) => reviewOfflineReceipt(prev, result, offline))
      return
    }

    let matches: EkasaRuleMatches
    try {
      matches = await matchEkasaRules(matchPayload(result.processedItems, state.rules))
    } catch (err) {
      if (processingRef.current !== promise) return
      toast.error(extractErrorMessage(err))
      return
    }
    if (processingRef.current !== promise) return
    for (const problem of matches.invalidRules) {
      toast.warning(t('ekasaImport.rulesStep.invalidRule', problem))
    }

    setState((prev) => ({
      ...prev,
      receiptData: result.receiptData,
      processedItems: result.processedItems,
      processedLegs: legsFromMatches(result.processedItems, matches, prev.defaultDeductiblePct, prev.defaultVatReclaimablePct, result.receiptDate),
      receiptDate: result.receiptDate,
      vendorName: result.receiptData?.organization.name ?? '',
      offlineFallback: result.offlineFallback,
      sourceRef: result.sourceRef,
      alreadyImported: result.alreadyImported,
      isProcessing: false,
      step: 'review',
    }))
  }, [state.rules, t])

  const handleBack = useCallback(() => {
    setState((prev) => {
      if (prev.step === 'rules') return { ...prev, step: 'upload' }
      if (prev.step === 'review') return { ...prev, step: 'rules' }
      return prev
    })
  }, [])

  const handleTryAgain = useCallback(() => {
    processingRef.current = null
    setState((prev) => ({
      ...prev,
      step: 'upload',
      filePath: null,
      receiptError: null,
      offlineFallback: null,
      isProcessing: false,
    }))
  }, [])

  const loadProfile = useCallback(async (personId: number) => {
    let profile: Awaited<ReturnType<typeof getEkasaProfile>>
    try {
      profile = await getEkasaProfile(personId)
    } catch (err) {
      toast.error(extractErrorMessage(err))
      return
    }
    if (!profile) return
    const rules = profile.rules.map((r) => ({
      id: crypto.randomUUID(),
      namePattern: r.namePattern,
      deductiblePct: bpsToPct(r.deductiblePctBps),
      vatReclaimablePct: bpsToPct(r.vatReclaimablePctBps),
    }))
    const defaultDeductiblePct = bpsToPct(profile.defaultDeductiblePctBps)
    const defaultVatReclaimablePct = bpsToPct(profile.defaultVatReclaimablePctBps)
    originalProfileRef.current = {
      rules: rules.map((r) => ({ namePattern: r.namePattern, deductiblePct: r.deductiblePct, vatReclaimablePct: r.vatReclaimablePct })),
      defaultDeductiblePct,
      defaultVatReclaimablePct,
    }
    setState((prev) => ({ ...prev, rules, defaultDeductiblePct, defaultVatReclaimablePct }))
  }, [])

  const saveProfile = useCallback(
    async (personId: number) => {
      await upsertEkasaProfile({
        personId,
        defaultDeductiblePctBps: pctToBps(state.defaultDeductiblePct) ?? 0,
        defaultVatReclaimablePctBps: pctToBps(state.defaultVatReclaimablePct) ?? 0,
        rules: state.rules
          .filter((r) => r.namePattern.trim() !== '')
          .map((r, i) => ({
            sortOrder: i,
            namePattern: r.namePattern,
            deductiblePctBps: pctToBps(r.deductiblePct) ?? 0,
            vatReclaimablePctBps: pctToBps(r.vatReclaimablePct) ?? 0,
          })),
      })
    },
    [state.defaultDeductiblePct, state.defaultVatReclaimablePct, state.rules],
  )

  const setRules = useCallback((rules: EkasaRuleDraft[]) => {
    setState((prev) => ({ ...prev, rules }))
  }, [])

  const setDefaultDeductiblePct = useCallback((pct: string) => {
    setState((prev) => ({ ...prev, defaultDeductiblePct: pct }))
  }, [])

  const setDefaultVatReclaimablePct = useCallback((pct: string) => {
    setState((prev) => ({ ...prev, defaultVatReclaimablePct: pct }))
  }, [])

  const profileChanged = useCallback((): boolean => {
    const current = snapshotFromState(state)
    const original = originalProfileRef.current
    return JSON.stringify(current) !== JSON.stringify(original)
  }, [state])

  const handleImportComplete = useCallback(() => {
    setState((prev) => ({ ...prev, step: 'save-profile' }))
  }, [])

  return {
    state,
    handleFileSelect,
    handleRulesConfirm,
    handleBack,
    handleTryAgain,
    loadProfile,
    saveProfile,
    handleImportComplete,
    profileChanged,
    setRules,
    setDefaultDeductiblePct,
    setDefaultVatReclaimablePct,
  }
}
