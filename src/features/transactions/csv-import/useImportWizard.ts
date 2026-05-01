import { useState, useCallback, useMemo, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { SnapshotRow, Currency, EventWithData, ImportProfileRow, ImportRule } from '../../../shared/types/index'
import {
  listEvents,
  listPartnerAccounts,
  listCurrencies,
  createPartnerAccount,
  updateAccount,
  updateEvent,
  bulkCreateCashflows,
  createSplitGroup,
  listImportProfiles,
  getPreferredProfile,
  setPreferredProfile,
  createImportProfile,
  updateImportProfile,
} from '../../../shared/api'
import { toMinorUnits } from '../../../shared/utils/format'
import { extractErrorMessage } from '../../../shared/utils/errors'
import type { WizardState, CashflowFieldKey, ImportRow, ColumnMapping, IbanMatchResult, SplitLeg } from './types'
import type { IbanLookupEntry } from './ibanMatcher'
import { buildIbanLookup, matchIban, normalizeIban } from './ibanMatcher'
import { parseCsvFile, autoDetectMapping, parseAmount, parseDateString } from './csvParser'
import { applyRules } from './transformRules'

const INITIAL_COLUMN_MAPPING: ColumnMapping = {
  date: null,
  amount: null,
  partner: null,
  note: null,
  currency: null,
  fxRate: null,
}

const INITIAL_WIZARD_STATE: WizardState = {
  step: 'upload',
  file: null,
  csvHeaders: [],
  csvRows: [],
  selectedAccountId: null,
  columnMapping: { ...INITIAL_COLUMN_MAPPING },
  importRows: [],
  importing: false,
  rules: [],
  loadedProfileId: null,
  originalProfileColumnMapping: null,
  originalProfileRules: null,
  importedCount: null,
}

const MS_PER_DAY = 86_400_000
const NEAR_DATE_WINDOW_DAYS = 5

const matchProfileMapping = (savedMapping: ColumnMapping, csvHeaders: string[]): ColumnMapping => {
  const result: ColumnMapping = { ...savedMapping }
  const fields: CashflowFieldKey[] = ['date', 'amount', 'partner', 'note', 'currency', 'fxRate']
  for (const field of fields) {
    const saved = savedMapping[field]
    if (saved === null) {
      result[field] = null
      continue
    }
    const match = csvHeaders.find((h) => h.trim().toLowerCase() === saved.trim().toLowerCase())
    result[field] = match ?? null
  }
  return result
}

const matchProfileRules = (savedRules: ImportRule[], csvHeaders: string[]): ImportRule[] => {
  return savedRules.map((rule) => {
    if (rule.type === 'sign_from_column') {
      const match = csvHeaders.find((h) => h.trim().toLowerCase() === rule.typeColumn.trim().toLowerCase())
      return { ...rule, typeColumn: match ?? '' }
    }
    if (rule.type === 'override_date_from_description') {
      const match = csvHeaders.find((h) => h.trim().toLowerCase() === rule.descriptionColumn.trim().toLowerCase())
      return { ...rule, descriptionColumn: match ?? '' }
    }
    return rule
  })
}

const rateToMantissaExponent = (rate: number): { mantissa: number; exponent: number } => {
  const str = rate.toString()
  const dotIndex = str.indexOf('.')
  if (dotIndex === -1) return { mantissa: rate, exponent: 0 }
  const decimalPlaces = str.length - dotIndex - 1
  const mantissa = Math.round(rate * Math.pow(10, decimalPlaces))
  return { mantissa, exponent: -decimalPlaces }
}

export const useImportWizard = (params: { snapshot: SnapshotRow[]; onClose: () => void; onSuccess: () => Promise<void> }) => {
  const { snapshot, onClose, onSuccess } = params
  const { t } = useTranslation()

  const [wizardState, setWizardState] = useState<WizardState>(INITIAL_WIZARD_STATE)
  const [ibanLookup, setIbanLookup] = useState<Map<string, IbanLookupEntry>>(new Map())
  const [existingEvents, setExistingEvents] = useState<EventWithData[]>([])
  const [currencies, setCurrencies] = useState<Currency[]>([])
  const [balanceWarningDates, setBalanceWarningDates] = useState<string[]>([])
  const [availableProfiles, setAvailableProfiles] = useState<ImportProfileRow[]>([])

  // Derived: selected account row from snapshot
  const selectedAccount = snapshot.find((r) => r.accountId === wizardState.selectedAccountId) ?? null
  const selectedAccountCurrencyCode = selectedAccount?.currencyCode ?? ''
  const selectedAccountMinorUnits = selectedAccount?.currencyMinorUnits ?? 2
  const selectedAccountCurrencyId = currencies.find((c) => c.code === selectedAccountCurrencyCode)?.id ?? 0

  // Derived: bucket rows that don't already have this account linked
  const availableBuckets = snapshot.filter((r) => r.accountType === 'bucket' && !r.bucketLinks.some((l) => l.sourceAccountId === wizardState.selectedAccountId))

  // Derived: account-type rows that have no IBAN set
  const accountsWithoutIban = snapshot.filter((r) => r.accountType === 'account' && !r.iban)

  // Derived: selection counts
  const selectedCount = wizardState.importRows.filter((r) => r.isSelected).length
  const duplicateCount = wizardState.importRows.filter((r) => r.isDuplicate || r.nearDateDuplicateEventId !== null).length
  const nearDateSkippedCount = wizardState.importRows.filter((r) => r.nearDateDuplicateEventId !== null && !r.isSelected).length

  useEffect(() => {
    listImportProfiles()
      .then(setAvailableProfiles)
      .catch(() => {})
  }, [])

  // ── Step 1 actions ──────────────────────────────────────────────────────────

  const handleFileSelect = useCallback(
    async (file: File) => {
      if (!file.name.toLowerCase().endsWith('.csv')) {
        toast.error(t('import.uploadStep.invalidFile'))
        return
      }
      try {
        const { headers, rows } = await parseCsvFile(file)
        setWizardState((prev) => ({ ...prev, file, csvHeaders: headers, csvRows: rows }))
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    },
    [t],
  )

  const handleAccountSelect = useCallback(
    async (accountId: number) => {
      setWizardState((prev) => ({ ...prev, selectedAccountId: accountId }))
      try {
        const [{ events }, partners, loadedCurrencies] = await Promise.all([listEvents({ accountId }), listPartnerAccounts(), listCurrencies()])
        setExistingEvents(events)
        setCurrencies(loadedCurrencies)
        setIbanLookup(buildIbanLookup(snapshot, partners))
      } catch (err) {
        toast.error(t('errors.loadData', { error: extractErrorMessage(err) }))
      }
      try {
        const profile = await getPreferredProfile(accountId)
        if (profile) {
          setWizardState((prev) => {
            const savedMapping = JSON.parse(profile.columnMappingJson) as ColumnMapping
            const resolvedMapping = matchProfileMapping(savedMapping, prev.csvHeaders)
            const parsedRules = profile.rules.map((r) => ({
              type: r.ruleType,
              sortOrder: r.sortOrder,
              ...JSON.parse(r.paramsJson),
            })) as ImportRule[]
            const resolvedRules = matchProfileRules(parsedRules, prev.csvHeaders)
            return {
              ...prev,
              loadedProfileId: profile.id,
              columnMapping: resolvedMapping,
              rules: resolvedRules,
              originalProfileColumnMapping: resolvedMapping,
              originalProfileRules: resolvedRules,
            }
          })
        }
      } catch {
        // swallow silently — profiles are a non-critical enhancement
      }
    },
    [snapshot, t],
  )

  const canProceedToMapping = wizardState.csvHeaders.length > 0 && wizardState.selectedAccountId !== null

  const goToMapping = useCallback(() => {
    setWizardState((prev) => {
      const newMapping = prev.loadedProfileId !== null ? prev.columnMapping : autoDetectMapping(prev.csvHeaders)
      return { ...prev, step: 'mapping', columnMapping: newMapping }
    })
  }, [])

  // ── Step 2 actions ──────────────────────────────────────────────────────────

  const handleMappingChange = useCallback((field: CashflowFieldKey, csvColumn: string | null) => {
    setWizardState((prev) => ({
      ...prev,
      columnMapping: { ...prev.columnMapping, [field]: csvColumn },
    }))
  }, [])

  const canProceedToReview = wizardState.columnMapping.date !== null && wizardState.columnMapping.amount !== null

  const goToRules = useCallback(() => {
    if (!wizardState.columnMapping.date || !wizardState.columnMapping.amount) return
    setWizardState((prev) => ({ ...prev, step: 'rules' }))
  }, [wizardState.columnMapping.date, wizardState.columnMapping.amount])

  const setRules = useCallback((rules: ImportRule[]) => {
    const renormalized = rules.map((r, i) => ({ ...r, sortOrder: i }))
    setWizardState((prev) => ({ ...prev, rules: renormalized }))
  }, [])

  const handleProfileSelect = useCallback(
    (profileId: number | null) => {
      if (profileId === null) {
        setWizardState((prev) => ({
          ...prev,
          loadedProfileId: null,
          originalProfileColumnMapping: null,
          originalProfileRules: null,
          columnMapping: autoDetectMapping(prev.csvHeaders),
          rules: [],
        }))
        return
      }
      const profile = availableProfiles.find((p) => p.id === profileId)
      if (!profile) return
      setWizardState((prev) => {
        const savedMapping = JSON.parse(profile.columnMappingJson) as ColumnMapping
        const resolvedMapping = matchProfileMapping(savedMapping, prev.csvHeaders)
        const parsedRules = profile.rules.map((r) => ({
          type: r.ruleType,
          sortOrder: r.sortOrder,
          ...JSON.parse(r.paramsJson),
        })) as ImportRule[]
        const resolvedRules = matchProfileRules(parsedRules, prev.csvHeaders)
        return {
          ...prev,
          loadedProfileId: profile.id,
          columnMapping: resolvedMapping,
          rules: resolvedRules,
          originalProfileColumnMapping: resolvedMapping,
          originalProfileRules: resolvedRules,
        }
      })
    },
    [availableProfiles],
  )

  const goToReview = useCallback(() => {
    const { csvRows, columnMapping, selectedAccountId, rules } = wizardState
    if (!selectedAccountId) return

    // Split existing events by type for duplicate detection vs balance warning
    const cashflowEvents = existingEvents.filter((e) => e.eventType === 'cashflow' || e.eventType === 'transfer')
    const balanceUpdateDateSet = new Set(existingEvents.filter((e) => e.eventType === 'balance_update').map((e) => e.eventDate.substring(0, 10)))

    // Build map of split group totals for duplicate detection
    const groupedEvents = new Map<number, { dateKey: string; total: number }>()
    for (const e of cashflowEvents) {
      if (e.splitGroupId !== null) {
        const existing = groupedEvents.get(e.splitGroupId)
        if (existing) {
          existing.total += e.amountMinor
        } else {
          groupedEvents.set(e.splitGroupId, {
            dateKey: e.eventDate.substring(0, 10),
            total: e.amountMinor,
          })
        }
      }
    }
    const groupTotalKeys = new Map<string, boolean>()
    for (const { dateKey, total } of groupedEvents.values()) {
      groupTotalKeys.set(`${dateKey}::${total}`, true)
    }

    const importRows: ImportRow[] = []
    const transformedRows = applyRules(csvRows, rules, columnMapping)

    const nearDateTransferCandidates = cashflowEvents.filter((e) => e.eventType === 'transfer' && e.splitGroupId === null)

    for (let i = 0; i < transformedRows.length; i++) {
      const csvRow = transformedRows[i]
      const rawDate = columnMapping.date ? (csvRow[columnMapping.date] ?? '') : ''
      const rawAmount = columnMapping.amount ? (csvRow[columnMapping.amount] ?? '') : ''
      const rawPartner = columnMapping.partner ? (csvRow[columnMapping.partner] ?? '') : ''
      const rawCurrency = columnMapping.currency ? (csvRow[columnMapping.currency] ?? '') : ''
      const rawFxRate = columnMapping.fxRate ? (csvRow[columnMapping.fxRate] ?? '') : ''
      const rawNote = columnMapping.note ? (csvRow[columnMapping.note] ?? '') : ''

      const parsedDate = csvRow.__overrideDateString ?? parseDateString(rawDate)
      const parsedAmount = parseAmount(rawAmount)
      if (!parsedDate || parsedAmount === null) continue

      let amountMinor = toMinorUnits(String(parsedAmount), selectedAccountMinorUnits)
      if (csvRow.__negateAmount) amountMinor = -amountMinor

      // IBAN matching
      let rawIban: string | null = null
      let ibanMatch: ImportRow['ibanMatch'] = { type: 'none' }
      if (rawPartner.trim() !== '') {
        rawIban = rawPartner.trim()
        ibanMatch = matchIban(normalizeIban(rawIban), ibanLookup)
      }
      // Suppress self-IBAN: the importing account's own IBAN in the partner column
      // is not a transfer — treat the row as if no IBAN was present.
      if (ibanMatch.type === 'ownAccount' && ibanMatch.accountId === selectedAccountId) {
        ibanMatch = { type: 'none' }
        rawIban = null
      }

      // Foreign currency handling
      let originalAmountMinor: number | null = null
      let originalCurrencyCode: string | null = null
      let fxRateMantissa: number | null = null
      let fxRateExponent: number | null = null
      const csvCurrencyCode = rawCurrency.trim().toUpperCase()
      if (csvCurrencyCode !== '' && csvCurrencyCode !== selectedAccountCurrencyCode.toUpperCase()) {
        const foreignCurrency = currencies.find((c) => c.code === csvCurrencyCode)
        if (foreignCurrency) {
          originalCurrencyCode = foreignCurrency.code
          originalAmountMinor = toMinorUnits(String(parsedAmount), foreignCurrency.minorUnits)
        }
      }

      // FX rate
      if (rawFxRate.trim() !== '') {
        const parsedRate = parseAmount(rawFxRate)
        if (parsedRate !== null && parsedRate > 0) {
          const { mantissa, exponent } = rateToMantissaExponent(parsedRate)
          fxRateMantissa = mantissa
          fxRateExponent = exponent
        }
      }

      // Duplicate detection: match YYYY-MM-DD and amount against existing cashflow/transfer events
      const datePrefix = parsedDate.substring(0, 10)
      const isDuplicate =
        cashflowEvents.some((e) => e.splitGroupId === null && e.eventDate.substring(0, 10) === datePrefix && e.amountMinor === amountMinor) ||
        groupTotalKeys.has(`${datePrefix}::${amountMinor}`)

      let nearDateDuplicateEventId: number | null = null
      if (!isDuplicate) {
        const rowDateMs = new Date(datePrefix).getTime()
        const nearDateMatches = nearDateTransferCandidates
          .map((e) => {
            const dateOnly = e.eventDate.substring(0, 10)
            const diffDays = Math.abs(new Date(dateOnly).getTime() - rowDateMs) / MS_PER_DAY
            return { event: e, dateOnly, diffDays }
          })
          .filter(({ dateOnly, diffDays, event: e }) => dateOnly !== datePrefix && e.amountMinor === amountMinor && diffDays <= NEAR_DATE_WINDOW_DAYS)
          .sort((a, b) => (a.diffDays !== b.diffDays ? a.diffDays - b.diffDays : b.event.createdAt.localeCompare(a.event.createdAt)))
          .map(({ event }) => event)
        nearDateDuplicateEventId = nearDateMatches[0]?.id ?? null
      }

      importRows.push({
        index: i,
        date: parsedDate,
        amountMinor,
        currencyCode: selectedAccountCurrencyCode,
        originalAmountMinor,
        originalCurrencyCode,
        fxRateMantissa,
        fxRateExponent,
        note: rawNote.trim() !== '' ? rawNote.trim() : null,
        rawIban,
        ibanMatch,
        isDuplicate,
        nearDateDuplicateEventId,
        isSelected: !isDuplicate && nearDateDuplicateEventId === null,
        bucketId: null,
        counterpartAccountId: null,
        splitLegs: null,
      })
    }

    // Balance warning: CSV row dates that also have a balance_update event
    const csvDateSet = new Set(importRows.map((r) => r.date.substring(0, 10)))
    const warnings = [...csvDateSet].filter((d) => balanceUpdateDateSet.has(d))

    setBalanceWarningDates(warnings)
    setWizardState((prev) => ({ ...prev, step: 'review', importRows }))
  }, [wizardState, existingEvents, ibanLookup, currencies, selectedAccountCurrencyCode, selectedAccountMinorUnits])

  // ── Step 3 actions ──────────────────────────────────────────────────────────

  const handleToggleRow = useCallback((index: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => (r.index === index ? { ...r, isSelected: !r.isSelected } : r)),
    }))
  }, [])

  const handleSelectAll = useCallback(() => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => ({ ...r, isSelected: true })),
    }))
  }, [])

  const handleDeselectAll = useCallback(() => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => ({ ...r, isSelected: false })),
    }))
  }, [])

  const handleBucketChange = useCallback((index: number, bucketId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => (r.index === index ? { ...r, bucketId } : r)),
    }))
  }, [])

  const handleCounterpartChange = useCallback(
    (index: number, accountId: number | null) => {
      setWizardState((prev) => ({
        ...prev,
        importRows: prev.importRows.map((r) => {
          if (r.index !== index) return r
          if (accountId === null) {
            return { ...r, counterpartAccountId: null, ibanMatch: { type: 'none' } }
          }
          const account = snapshot.find((s) => s.accountId === accountId)
          if (!account) return r
          const ibanMatch: ImportRow['ibanMatch'] =
            account.accountType === 'account'
              ? { type: 'ownAccount', accountId, accountName: account.accountName }
              : { type: 'partner', accountId, accountName: account.accountName }
          return { ...r, counterpartAccountId: accountId, ibanMatch }
        }),
      }))
    },
    [snapshot],
  )

  const handleCreatePartner = useCallback(
    async (iban: string, name: string) => {
      if (!selectedAccountCurrencyId) return
      const normalizedIban = normalizeIban(iban)
      try {
        const newId = await createPartnerAccount(name, selectedAccountCurrencyId, normalizedIban)
        const entry: IbanLookupEntry = {
          accountId: newId,
          accountName: name,
          accountType: 'partner',
          iban: normalizedIban,
        }
        setIbanLookup((prev) => new Map(prev).set(normalizedIban, entry))
        const newIbanMatch: IbanMatchResult = {
          type: 'partner',
          accountId: newId,
          accountName: name,
        }
        setWizardState((prev) => ({
          ...prev,
          importRows: prev.importRows.map((r) => {
            const topMatch = r.rawIban && normalizeIban(r.rawIban) === normalizedIban ? { ibanMatch: newIbanMatch } : {}
            const updatedLegs =
              r.splitLegs !== null
                ? r.splitLegs.map((l) => (l.rawIban && normalizeIban(l.rawIban) === normalizedIban ? { ...l, ibanMatch: newIbanMatch, counterpartAccountId: newId } : l))
                : r.splitLegs
            if (Object.keys(topMatch).length === 0 && updatedLegs === r.splitLegs) return r
            return { ...r, ...topMatch, splitLegs: updatedLegs }
          }),
        }))
        toast.success(t('import.reviewStep.partnerCreated'))
      } catch (err) {
        toast.error(t('errors.createPartner', { error: extractErrorMessage(err) }))
      }
    },
    [selectedAccountCurrencyId, t],
  )

  const handleAssignIban = useCallback(
    async (iban: string, targetAccountId: number) => {
      const targetRow = snapshot.find((r) => r.accountId === targetAccountId)
      if (!targetRow) return
      const normalizedIban = normalizeIban(iban)
      try {
        await updateAccount(targetAccountId, targetRow.accountName, normalizedIban)
        const entry: IbanLookupEntry = {
          accountId: targetAccountId,
          accountName: targetRow.accountName,
          accountType: 'account',
          iban: normalizedIban,
        }
        setIbanLookup((prev) => new Map(prev).set(normalizedIban, entry))
        const newIbanMatch: IbanMatchResult = {
          type: 'ownAccount',
          accountId: targetAccountId,
          accountName: targetRow.accountName,
        }
        setWizardState((prev) => ({
          ...prev,
          importRows: prev.importRows.map((r) => {
            const topMatch = r.rawIban && normalizeIban(r.rawIban) === normalizedIban ? { ibanMatch: newIbanMatch } : {}
            const updatedLegs =
              r.splitLegs !== null
                ? r.splitLegs.map((l) => (l.rawIban && normalizeIban(l.rawIban) === normalizedIban ? { ...l, ibanMatch: newIbanMatch, counterpartAccountId: targetAccountId } : l))
                : r.splitLegs
            if (Object.keys(topMatch).length === 0 && updatedLegs === r.splitLegs) return r
            return { ...r, ...topMatch, splitLegs: updatedLegs }
          }),
        }))
        toast.success(t('import.reviewStep.accountAssigned'))
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    },
    [snapshot, t],
  )

  const handleSplitOpen = useCallback((rowIndex: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex) return r
        const leg0: SplitLeg = {
          legIndex: 0,
          amountMinor: r.amountMinor,
          note: null,
          rawIban: r.rawIban,
          ibanMatch: r.ibanMatch,
          bucketId: r.bucketId,
          counterpartAccountId: r.counterpartAccountId,
        }
        return { ...r, splitLegs: [leg0] }
      }),
    }))
  }, [])

  const handleSplitCancel = useCallback((rowIndex: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => (r.index === rowIndex ? { ...r, splitLegs: null } : r)),
    }))
  }, [])

  const handleLegAmountChange = useCallback((rowIndex: number, legIndex: number, amountMinor: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legIndex === legIndex ? { ...l, amountMinor } : l)),
        }
      }),
    }))
  }, [])

  const handleLegNoteChange = useCallback((rowIndex: number, legIndex: number, note: string | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legIndex === legIndex ? { ...l, note } : l)),
        }
      }),
    }))
  }, [])

  const handleLegPartnerChange = useCallback((rowIndex: number, legIndex: number, rawIban: string | null, ibanMatch: IbanMatchResult, counterpartAccountId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legIndex === legIndex ? { ...l, rawIban, ibanMatch, counterpartAccountId } : l)),
        }
      }),
    }))
  }, [])

  const handleLegBucketChange = useCallback((rowIndex: number, legIndex: number, bucketId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legIndex === legIndex ? { ...l, bucketId } : l)),
        }
      }),
    }))
  }, [])

  const handleAddLeg = useCallback((rowIndex: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        const allocated = r.splitLegs.reduce((sum, l) => sum + l.amountMinor, 0)
        const newLeg: SplitLeg = {
          legIndex: r.splitLegs.length,
          amountMinor: r.amountMinor - allocated,
          note: null,
          rawIban: null,
          ibanMatch: { type: 'none' },
          bucketId: null,
          counterpartAccountId: null,
        }
        return { ...r, splitLegs: [...r.splitLegs, newLeg] }
      }),
    }))
  }, [])

  const handleRemoveLeg = useCallback((rowIndex: number, legIndex: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null || r.splitLegs.length <= 1) return r
        const filtered = r.splitLegs.filter((l) => l.legIndex !== legIndex)
        return { ...r, splitLegs: filtered.map((l, i) => ({ ...l, legIndex: i })) }
      }),
    }))
  }, [])

  const handleImport = useCallback(async () => {
    const { selectedAccountId, importRows } = wizardState
    if (!selectedAccountId) return

    setWizardState((prev) => ({ ...prev, importing: true }))
    try {
      const selectedRows = importRows.filter((r) => r.isSelected && !r.isDuplicate)
      const regularRows = selectedRows.filter((r) => r.splitLegs === null)
      const splitRows = selectedRows.filter((r) => r.splitLegs !== null)

      // Phase 0: Update dates for skipped near-date duplicates
      const nearDateSkipped = importRows.filter((r) => r.nearDateDuplicateEventId !== null && !r.isSelected)
      for (const row of nearDateSkipped) {
        const matchedEvent = existingEvents.find((e) => e.id === row.nearDateDuplicateEventId)
        if (matchedEvent) {
          await updateEvent(matchedEvent.id, matchedEvent.amountMinor, row.date, matchedEvent.note ?? undefined)
        }
      }

      if (regularRows.length > 0) {
        const entries = regularRows.map((row) => {
          const counterpartAccountId = row.ibanMatch.type === 'ownAccount' || row.ibanMatch.type === 'partner' ? row.ibanMatch.accountId : undefined

          const originalCurrencyId = row.originalCurrencyCode ? currencies.find((c) => c.code === row.originalCurrencyCode)?.id : undefined

          return {
            accountId: selectedAccountId,
            amountMinor: row.amountMinor,
            eventDate: row.date,
            note: row.note ?? undefined,
            counterpartAccountId,
            bucketId: row.bucketId ?? undefined,
            originalCurrencyId,
            originalAmountMinor: row.originalAmountMinor ?? undefined,
            fxRateMantissa: row.fxRateMantissa ?? undefined,
            fxRateExponent: row.fxRateExponent ?? undefined,
          }
        })

        await bulkCreateCashflows({ entries })
      }

      for (const splitRow of splitRows) {
        const originalCurrencyId = splitRow.originalCurrencyCode ? (currencies.find((c) => c.code === splitRow.originalCurrencyCode)?.id ?? null) : null

        await createSplitGroup({
          accountId: selectedAccountId,
          groupNote: splitRow.note ?? null,
          legs: splitRow.splitLegs!.map((leg) => ({
            amountMinor: leg.amountMinor,
            eventDate: splitRow.date,
            note: leg.note ?? null,
            counterpartAccountId: leg.counterpartAccountId ?? null,
            bucketId: leg.bucketId ?? null,
            originalCurrencyId: originalCurrencyId ?? null,
            originalAmountMinor: splitRow.originalAmountMinor ?? null,
            fxRateMantissa: splitRow.fxRateMantissa ?? null,
            fxRateExponent: splitRow.fxRateExponent ?? null,
          })),
        })
      }

      const totalImported = regularRows.length + splitRows.reduce((n, r) => n + r.splitLegs!.length, 0)

      const { columnMapping, rules, loadedProfileId, originalProfileColumnMapping, originalProfileRules } = wizardState
      const profileChanged = JSON.stringify(columnMapping) !== JSON.stringify(originalProfileColumnMapping) || JSON.stringify(rules) !== JSON.stringify(originalProfileRules)

      if (loadedProfileId !== null) {
        setPreferredProfile(selectedAccountId, loadedProfileId).catch(() => {})
      }

      if (loadedProfileId === null || profileChanged) {
        setWizardState((prev) => ({ ...prev, step: 'save-profile', importedCount: totalImported }))
      } else {
        toast.success(t('import.success', { count: totalImported }))
        await onSuccess()
        onClose()
      }
    } catch (err) {
      toast.error(t('errors.importCashflows', { error: extractErrorMessage(err) }))
    } finally {
      setWizardState((prev) => ({ ...prev, importing: false }))
    }
  }, [wizardState, existingEvents, currencies, t, onSuccess, onClose])

  const goBack = useCallback(() => {
    setWizardState((prev) => {
      if (prev.step === 'review') return { ...prev, step: 'rules' }
      if (prev.step === 'rules') return { ...prev, step: 'mapping' }
      if (prev.step === 'mapping') return { ...prev, step: 'upload' }
      return prev
    })
  }, [])

  const handleSaveNewProfile = useCallback(
    async (profileName: string) => {
      const { columnMapping, rules, selectedAccountId: accountId } = wizardState
      const columnMappingJson = JSON.stringify(columnMapping)
      const ruleInputs = rules.map((rule) => {
        const { type, sortOrder, ...params } = rule
        return { ruleType: type, sortOrder, paramsJson: JSON.stringify(params) }
      })
      try {
        const newId = await createImportProfile(profileName, columnMappingJson, ruleInputs)
        if (accountId !== null) {
          setPreferredProfile(accountId, newId).catch(() => {})
        }
        toast.success(t('import.saveStep.profileSaved'))
        await onSuccess()
        onClose()
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    },
    [wizardState, t, onSuccess, onClose],
  )

  const handleUpdateProfile = useCallback(async () => {
    const { columnMapping, rules, loadedProfileId } = wizardState
    if (loadedProfileId === null) return
    const originalProfileName = availableProfiles.find((p) => p.id === loadedProfileId)?.name ?? ''
    const columnMappingJson = JSON.stringify(columnMapping)
    const ruleInputs = rules.map((rule) => {
      const { type, sortOrder, ...params } = rule
      return { ruleType: type, sortOrder, paramsJson: JSON.stringify(params) }
    })
    try {
      await updateImportProfile(loadedProfileId, originalProfileName, columnMappingJson, ruleInputs)
      await onSuccess()
      onClose()
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }, [wizardState, availableProfiles, onSuccess, onClose])

  const handleSkipSaveProfile = useCallback(async () => {
    await onSuccess()
    onClose()
  }, [onSuccess, onClose])

  const splitValidationErrors = useMemo(
    () =>
      wizardState.importRows
        .filter((r) => r.splitLegs !== null && r.isSelected)
        .filter((r) => r.splitLegs!.reduce((sum, l) => sum + l.amountMinor, 0) !== r.amountMinor)
        .map((r) => t('import.reviewStep.split.validationError', { date: r.date.substring(0, 10) })),
    [wizardState.importRows, t],
  )

  return {
    wizardState,
    ibanLookup,
    availableBuckets,
    availableProfiles,
    selectedCount,
    duplicateCount,
    nearDateSkippedCount,
    balanceWarningDates,
    accountsWithoutIban,
    selectedAccountCurrencyCode,
    selectedAccountCurrencyId,
    selectedAccountMinorUnits,
    splitValidationErrors,
    handleFileSelect,
    handleAccountSelect,
    canProceedToMapping,
    goToMapping,
    handleMappingChange,
    canProceedToReview,
    goToRules,
    goToReview,
    setRules,
    handleProfileSelect,
    handleToggleRow,
    handleSelectAll,
    handleDeselectAll,
    handleBucketChange,
    handleCounterpartChange,
    handleCreatePartner,
    handleAssignIban,
    handleSplitOpen,
    handleSplitCancel,
    handleLegAmountChange,
    handleLegNoteChange,
    handleLegPartnerChange,
    handleLegBucketChange,
    handleAddLeg,
    handleRemoveLeg,
    handleImport,
    handleSaveNewProfile,
    handleUpdateProfile,
    handleSkipSaveProfile,
    goBack,
  }
}
