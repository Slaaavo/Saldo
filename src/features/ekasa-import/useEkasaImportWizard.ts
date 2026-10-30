import { useState, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EkasaWizardState, EkasaRuleDraft, EkasaRuleMatches, MatchEkasaRulesPayload, ProcessReceiptResult, ReceiptReadLine } from './types'
import { makeEmptyLeg } from '../transactions/splitLegDraft'
import type { SplitLegDraft } from '../transactions/splitLegDraft'
import { fromMinorUnits, bpsToPct, pctToBps } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { getEkasaProfile, upsertEkasaProfile, processReceiptFile, matchEkasaRules } from '../../shared/api'

const INITIAL_STATE: EkasaWizardState = {
  step: 'upload',
  filePath: null,
  processedLegs: [],
  receiptError: null,
  rules: [],
  defaultDeductiblePct: '100',
  defaultVatReclaimablePct: '100',
  isProcessing: false,
  vendorName: '',
  receiptDate: '',
  sourceRef: null,
  alreadyImported: false,
}

const matchPayload = (lines: ReceiptReadLine[], rules: EkasaRuleDraft[]): MatchEkasaRulesPayload => ({
  itemNames: lines.map((line) => line.name),
  patterns: rules.map((rule) => rule.namePattern),
})

// A matched leg takes its rule row's percentages as typed, so a cleared one stays blank.
const legsFromMatches = (
  lines: ReceiptReadLine[],
  rules: EkasaRuleDraft[],
  matches: EkasaRuleMatches,
  defaultDeductiblePct: string,
  defaultVatReclaimablePct: string,
  date: string,
): SplitLegDraft[] =>
  lines.map((line, index) => {
    const row = matches.matchedRows[index] ?? null
    const rule = row === null ? undefined : rules[row - 1]
    return {
      ...makeEmptyLeg(date),
      amount: fromMinorUnits(line.grossMinor, 2),
      vatRate: bpsToPct(line.vatRateBps),
      note: line.name,
      expenseDeductiblePct: rule ? rule.deductiblePct : defaultDeductiblePct,
      vatReclaimablePct: rule ? rule.vatReclaimablePct : defaultVatReclaimablePct,
    }
  })

interface ProfileSnapshot {
  rules: Array<{ namePattern: string; deductiblePct: string; vatReclaimablePct: string }>
  defaultDeductiblePct: string
  defaultVatReclaimablePct: string
}

// Offline receipts carry no item detail, so the review step gets a single leg for the total.
const reviewOfflineReceipt = (prev: EkasaWizardState, result: ProcessReceiptResult): EkasaWizardState => {
  const leg: SplitLegDraft = {
    ...makeEmptyLeg(result.date),
    amount: fromMinorUnits(result.totalMinor, 2),
    expenseDeductiblePct: prev.defaultDeductiblePct,
    vatReclaimablePct: prev.defaultVatReclaimablePct,
  }
  return {
    ...prev,
    processedLegs: [leg],
    receiptDate: result.date,
    vendorName: '',
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

    if (result.kind === 'offline') {
      setState((prev) => reviewOfflineReceipt(prev, result))
      return
    }
    const { lines, vendor } = result
    const rules = state.rules

    let matches: EkasaRuleMatches
    try {
      matches = await matchEkasaRules(matchPayload(lines, rules))
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
      processedLegs: legsFromMatches(lines, rules, matches, prev.defaultDeductiblePct, prev.defaultVatReclaimablePct, result.date),
      receiptDate: result.date,
      vendorName: vendor,
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
