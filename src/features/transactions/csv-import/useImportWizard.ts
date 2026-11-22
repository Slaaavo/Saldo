import { useState, useCallback, useMemo, useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { SnapshotRow, Currency, ImportProfileRow, ImportRule, CsvImportPreview, ImportVerdict, IncomingRowInput, RowChoice } from '../../../shared/types/index'
import {
  previewCsvImport,
  applyCsvImport,
  listPartnerAccounts,
  listCurrencies,
  createPartnerAccount,
  updateAccount,
  listImportProfiles,
  getPreferredProfile,
  setPreferredProfile,
  createImportProfile,
  updateImportProfile,
} from '../../../shared/api'
import { toMinorUnits } from '../../../shared/utils/format'
import { extractErrorMessage, hasErrorCode } from '../../../shared/utils/errors'
import { takesEntries } from '../../../shared/utils/snapshot'
import type { WizardState, CashflowFieldKey, ImportRow, ParsedRow, ColumnMapping, IbanMatchResult, SplitLeg } from './types'
import type { IbanLookupEntry } from './ibanMatcher'
import { buildIbanLookup, matchIban, normalizeIban } from './ibanMatcher'
import { parseCsvFile, autoDetectMapping, parseAmount, parseDateString } from './csvParser'
import { applyRules } from './transformRules'
import { isNewRow } from './verdicts'

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
  unparseableRowCount: 0,
  reviewToken: null,
  balanceUpdateDays: [],
  previewing: false,
  importing: false,
  rules: [],
  loadedProfile: null,
  originalProfileColumnMapping: null,
  originalProfileRules: null,
  importedCount: null,
}

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

// A profile can be picked before the CSV, so callers re-run this whenever the headers change.
const resolveProfile = (profile: ImportProfileRow, csvHeaders: string[]): Partial<WizardState> => {
  const savedMapping = JSON.parse(profile.columnMappingJson) as ColumnMapping
  const columnMapping = matchProfileMapping(savedMapping, csvHeaders)
  const parsedRules = profile.rules.map((r) => ({
    type: r.ruleType,
    sortOrder: r.sortOrder,
    ...JSON.parse(r.paramsJson),
  })) as ImportRule[]
  const rules = matchProfileRules(parsedRules, csvHeaders)
  return {
    loadedProfile: profile,
    columnMapping,
    rules,
    originalProfileColumnMapping: columnMapping,
    originalProfileRules: rules,
  }
}

type AccountMatch = Extract<IbanMatchResult, { accountId: number }>

const counterpartOf = (match: IbanMatchResult): number | null => (match.type === 'ownAccount' || match.type === 'partner' ? match.accountId : null)

const toIncomingRow = (row: ParsedRow): IncomingRowInput => ({
  index: row.index,
  eventDate: row.date,
  amountMinor: row.amountMinor,
  note: row.note,
  counterpartAccountId: counterpartOf(row.ibanMatch),
  namesUnknownAccount: row.ibanMatch.type === 'unmatched',
  originalCurrencyId: row.originalCurrencyId,
  originalAmountMinor: row.originalAmountMinor,
  fxRateMantissa: row.fxRateMantissa,
  fxRateExponent: row.fxRateExponent,
})

const toChoice = (row: ImportRow): RowChoice => ({
  index: row.index,
  selected: row.isSelected,
  bucketId: row.bucketId,
  split:
    row.splitLegs !== null
      ? row.splitLegs.map((leg) => ({ amountMinor: leg.amountMinor, note: leg.note, counterpartAccountId: leg.counterpartAccountId, bucketId: leg.bucketId }))
      : null,
})

const withDefaultChoice = (row: ParsedRow, verdict: ImportVerdict): ImportRow => ({ ...row, verdict, isSelected: isNewRow(verdict), bucketId: null, splitLegs: null })

const verdictsByIndex = (preview: CsvImportPreview): Map<number, ImportVerdict> => new Map(preview.rows.map((row) => [row.index, row.verdict]))

const withVerdicts = (rows: ImportRow[], preview: CsvImportPreview): ImportRow[] => {
  const verdicts = verdictsByIndex(preview)
  return rows.map((row) => {
    const verdict = verdicts.get(row.index)!
    return verdict.kind === row.verdict.kind ? { ...row, verdict } : withDefaultChoice(row, verdict)
  })
}

const relinkIban = (rows: ImportRow[], normalizedIban: string, match: AccountMatch): ImportRow[] => {
  const hasIban = (rawIban: string | null) => rawIban !== null && normalizeIban(rawIban) === normalizedIban
  return rows.map((row) => ({
    ...row,
    ibanMatch: hasIban(row.rawIban) ? match : row.ibanMatch,
    splitLegs: row.splitLegs?.map((leg) => (hasIban(leg.rawIban) ? { ...leg, ibanMatch: match, counterpartAccountId: match.accountId } : leg)) ?? null,
  }))
}

const selectNewRows = (rows: ImportRow[], isSelected: boolean): ImportRow[] => rows.map((row) => (isNewRow(row.verdict) ? { ...row, isSelected } : row))

const rateToMantissaExponent = (rate: number): { mantissa: number; exponent: number } => {
  const str = rate.toString()
  const dotIndex = str.indexOf('.')
  if (dotIndex === -1) return { mantissa: rate, exponent: 0 }
  const decimalPlaces = str.length - dotIndex - 1
  const mantissa = Math.round(rate * Math.pow(10, decimalPlaces))
  return { mantissa, exponent: -decimalPlaces }
}

export const useImportWizard = (params: { snapshot: SnapshotRow[]; preselectedAccountId?: number; onClose: () => void; onSuccess: () => Promise<void> }) => {
  const { snapshot, preselectedAccountId, onClose, onSuccess } = params
  const { t } = useTranslation()

  const [wizardState, setWizardState] = useState<WizardState>(INITIAL_WIZARD_STATE)
  const [ibanLookup, setIbanLookup] = useState<Map<string, IbanLookupEntry>>(new Map())
  const [currencies, setCurrencies] = useState<Currency[]>([])
  const [availableProfiles, setAvailableProfiles] = useState<ImportProfileRow[]>([])
  // Responses for an account the user has since switched away from are dropped.
  const selectedAccountIdRef = useRef<number | null>(null)
  const preselectedAccountIdRef = useRef(preselectedAccountId)
  const previewSeqRef = useRef(0)

  // Derived: selected account row from snapshot
  const selectedAccount = snapshot.find((r) => r.accountId === wizardState.selectedAccountId) ?? null
  const selectedAccountCurrencyCode = selectedAccount?.currencyCode ?? ''
  const selectedAccountMinorUnits = selectedAccount?.currencyMinorUnits ?? 2
  const selectedAccountCurrencyId = currencies.find((c) => c.code === selectedAccountCurrencyCode)?.id ?? 0

  // Derived: bucket rows that don't already have this account linked
  const availableBuckets = snapshot.filter((r) => r.accountType === 'bucket' && takesEntries(r) && !r.bucketLinks.some((l) => l.sourceAccountId === wizardState.selectedAccountId))

  // Derived: account-type rows that have no IBAN set
  const accountsWithoutIban = snapshot.filter((r) => r.accountType === 'account' && !r.iban)

  const selectedCount = wizardState.importRows.filter((r) => r.isSelected).length

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
        setWizardState((prev) => ({
          ...prev,
          file,
          csvHeaders: headers,
          csvRows: rows,
          ...(prev.loadedProfile !== null ? resolveProfile(prev.loadedProfile, headers) : {}),
        }))
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    },
    [t],
  )

  const handleAccountSelect = useCallback(
    async (accountId: number) => {
      selectedAccountIdRef.current = accountId
      // The previous account's profile must not carry over when this one has no preferred profile.
      setWizardState((prev) => ({
        ...prev,
        selectedAccountId: accountId,
        loadedProfile: null,
        columnMapping: { ...INITIAL_COLUMN_MAPPING },
        rules: [],
        originalProfileColumnMapping: null,
        originalProfileRules: null,
      }))
      try {
        const [partners, loadedCurrencies] = await Promise.all([listPartnerAccounts(), listCurrencies()])
        setCurrencies(loadedCurrencies)
        setIbanLookup(buildIbanLookup(snapshot, partners))
      } catch (err) {
        toast.error(t('errors.loadData', { error: extractErrorMessage(err) }))
      }
      try {
        const profile = await getPreferredProfile(accountId)
        if (profile && selectedAccountIdRef.current === accountId) {
          setWizardState((prev) => ({ ...prev, ...resolveProfile(profile, prev.csvHeaders) }))
        }
      } catch {
        // swallow silently — profiles are a non-critical enhancement
      }
    },
    [snapshot, t],
  )

  useEffect(() => {
    const accountId = preselectedAccountIdRef.current
    if (accountId === undefined) return
    preselectedAccountIdRef.current = undefined
    void handleAccountSelect(accountId)
  }, [handleAccountSelect])

  const canProceedToMapping = wizardState.csvHeaders.length > 0 && wizardState.selectedAccountId !== null

  const goToMapping = useCallback(() => {
    setWizardState((prev) => {
      const newMapping = prev.loadedProfile !== null ? prev.columnMapping : autoDetectMapping(prev.csvHeaders)
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
          loadedProfile: null,
          originalProfileColumnMapping: null,
          originalProfileRules: null,
          columnMapping: autoDetectMapping(prev.csvHeaders),
          rules: [],
        }))
        return
      }
      const profile = availableProfiles.find((p) => p.id === profileId)
      if (!profile) return
      setWizardState((prev) => ({ ...prev, ...resolveProfile(profile, prev.csvHeaders) }))
    },
    [availableProfiles],
  )

  const previewUnlessSuperseded = useCallback(async (accountId: number, rows: ParsedRow[]): Promise<CsvImportPreview | null> => {
    const seq = ++previewSeqRef.current
    setWizardState((prev) => ({ ...prev, previewing: true }))
    try {
      const result = await previewCsvImport({ accountId, rows: rows.map(toIncomingRow) })
      return seq === previewSeqRef.current ? result : null
    } finally {
      if (seq === previewSeqRef.current) setWizardState((prev) => ({ ...prev, previewing: false }))
    }
  }, [])

  const previewAgain = useCallback(
    async (rows: ImportRow[]) => {
      const accountId = wizardState.selectedAccountId
      if (accountId === null || rows.length === 0) return
      try {
        const latest = await previewUnlessSuperseded(accountId, rows)
        if (latest === null) return
        setWizardState((prev) => ({
          ...prev,
          importRows: withVerdicts(prev.importRows, latest),
          reviewToken: latest.reviewToken,
          balanceUpdateDays: latest.balanceUpdateDays,
        }))
      } catch (err) {
        toast.error(t('errors.loadData', { error: extractErrorMessage(err) }))
      }
    },
    [wizardState.selectedAccountId, previewUnlessSuperseded, t],
  )

  const goToReview = useCallback(async () => {
    const { csvRows, columnMapping, selectedAccountId, rules } = wizardState
    if (!selectedAccountId) return

    const parsedRows: ParsedRow[] = []
    let unparseableRowCount = 0
    const transformedRows = applyRules(csvRows, rules, columnMapping)

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
      if (!parsedDate || parsedAmount === null) {
        unparseableRowCount++
        continue
      }

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
      let originalCurrencyId: number | null = null
      let fxRateMantissa: number | null = null
      let fxRateExponent: number | null = null
      const csvCurrencyCode = rawCurrency.trim().toUpperCase()
      if (csvCurrencyCode !== '' && csvCurrencyCode !== selectedAccountCurrencyCode.toUpperCase()) {
        const foreignCurrency = currencies.find((c) => c.code === csvCurrencyCode)
        if (foreignCurrency) {
          originalCurrencyId = foreignCurrency.id
          originalAmountMinor = toMinorUnits(String(parsedAmount), foreignCurrency.minorUnits)
          if (csvRow.__negateAmount) originalAmountMinor = -originalAmountMinor
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

      parsedRows.push({
        index: i,
        date: parsedDate,
        amountMinor,
        note: rawNote.trim() !== '' ? rawNote.trim() : null,
        originalCurrencyId,
        originalAmountMinor,
        fxRateMantissa,
        fxRateExponent,
        rawIban,
        ibanMatch,
      })
    }

    try {
      const latest = await previewUnlessSuperseded(selectedAccountId, parsedRows)
      if (latest === null) return
      const verdicts = verdictsByIndex(latest)
      setWizardState((prev) => ({
        ...prev,
        step: 'review',
        importRows: parsedRows.map((row) => withDefaultChoice(row, verdicts.get(row.index)!)),
        unparseableRowCount,
        reviewToken: latest.reviewToken,
        balanceUpdateDays: latest.balanceUpdateDays,
      }))
    } catch (err) {
      toast.error(t('errors.loadData', { error: extractErrorMessage(err) }))
    }
  }, [wizardState, ibanLookup, currencies, selectedAccountCurrencyCode, selectedAccountMinorUnits, previewUnlessSuperseded, t])

  // ── Step 3 actions ──────────────────────────────────────────────────────────

  const handleToggleRow = useCallback((index: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => (r.index === index ? { ...r, isSelected: !r.isSelected } : r)),
    }))
  }, [])

  const handleSelectAll = useCallback(() => {
    setWizardState((prev) => ({ ...prev, importRows: selectNewRows(prev.importRows, true) }))
  }, [])

  const handleDeselectAll = useCallback(() => {
    setWizardState((prev) => ({ ...prev, importRows: selectNewRows(prev.importRows, false) }))
  }, [])

  const handleBucketChange = useCallback((index: number, bucketId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => (r.index === index ? { ...r, bucketId } : r)),
    }))
  }, [])

  const handleCounterpartChange = useCallback(
    (index: number, accountId: number | null) => {
      const account = snapshot.find((s) => s.accountId === accountId)
      if (accountId !== null && !account) return
      const ibanMatch: IbanMatchResult = account ? { type: 'ownAccount', accountId: account.accountId, accountName: account.accountName } : { type: 'none' }
      const relink = (rows: ImportRow[]) => rows.map((r) => (r.index === index ? { ...r, ibanMatch } : r))
      setWizardState((prev) => ({ ...prev, importRows: relink(prev.importRows) }))
      void previewAgain(relink(wizardState.importRows))
    },
    [snapshot, wizardState.importRows, previewAgain],
  )

  const handleCreatePartner = useCallback(
    async (iban: string, name: string): Promise<boolean> => {
      if (!selectedAccountCurrencyId) {
        toast.error(t('import.reviewStep.partnerCurrencyMissing'))
        return false
      }
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
        const match: AccountMatch = { type: 'partner', accountId: newId, accountName: name }
        setWizardState((prev) => ({ ...prev, importRows: relinkIban(prev.importRows, normalizedIban, match) }))
        void previewAgain(relinkIban(wizardState.importRows, normalizedIban, match))
        toast.success(t('import.reviewStep.partnerCreated'))
        return true
      } catch (err) {
        toast.error(t('errors.createPartner', { error: extractErrorMessage(err) }))
        return false
      }
    },
    [selectedAccountCurrencyId, wizardState.importRows, previewAgain, t],
  )

  const handleAssignIban = useCallback(
    async (iban: string, targetAccountId: number) => {
      const targetRow = snapshot.find((r) => r.accountId === targetAccountId)
      if (!targetRow) return
      const normalizedIban = normalizeIban(iban)
      try {
        await updateAccount(
          targetAccountId,
          targetRow.accountName,
          normalizedIban,
          targetRow.personId,
          targetRow.purchasePriceMinor,
          targetRow.purchaseDate,
          targetRow.depreciationPeriodMonths,
        )
        const entry: IbanLookupEntry = {
          accountId: targetAccountId,
          accountName: targetRow.accountName,
          accountType: 'account',
          iban: normalizedIban,
        }
        setIbanLookup((prev) => new Map(prev).set(normalizedIban, entry))
        const match: AccountMatch = { type: 'ownAccount', accountId: targetAccountId, accountName: targetRow.accountName }
        setWizardState((prev) => ({ ...prev, importRows: relinkIban(prev.importRows, normalizedIban, match) }))
        void previewAgain(relinkIban(wizardState.importRows, normalizedIban, match))
        toast.success(t('import.reviewStep.accountAssigned'))
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    },
    [snapshot, wizardState.importRows, previewAgain, t],
  )

  const handleSplitOpen = useCallback((rowIndex: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex) return r
        const leg0: SplitLeg = {
          legId: 0,
          amountMinor: r.amountMinor,
          note: null,
          rawIban: r.rawIban,
          ibanMatch: r.ibanMatch,
          bucketId: r.bucketId,
          counterpartAccountId: counterpartOf(r.ibanMatch),
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

  const handleLegAmountChange = useCallback((rowIndex: number, legId: number, amountMinor: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legId === legId ? { ...l, amountMinor } : l)),
        }
      }),
    }))
  }, [])

  const handleLegNoteChange = useCallback((rowIndex: number, legId: number, note: string | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legId === legId ? { ...l, note } : l)),
        }
      }),
    }))
  }, [])

  const handleLegPartnerChange = useCallback((rowIndex: number, legId: number, rawIban: string | null, ibanMatch: IbanMatchResult, counterpartAccountId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legId === legId ? { ...l, rawIban, ibanMatch, counterpartAccountId } : l)),
        }
      }),
    }))
  }, [])

  const handleLegBucketChange = useCallback((rowIndex: number, legId: number, bucketId: number | null) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null) return r
        return {
          ...r,
          splitLegs: r.splitLegs.map((l) => (l.legId === legId ? { ...l, bucketId } : l)),
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
          legId: Math.max(...r.splitLegs.map((l) => l.legId)) + 1,
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

  const handleRemoveLeg = useCallback((rowIndex: number, legId: number) => {
    setWizardState((prev) => ({
      ...prev,
      importRows: prev.importRows.map((r) => {
        if (r.index !== rowIndex || r.splitLegs === null || r.splitLegs.length <= 1) return r
        return { ...r, splitLegs: r.splitLegs.filter((l) => l.legId !== legId) }
      }),
    }))
  }, [])

  const handleImport = useCallback(async () => {
    const { selectedAccountId, importRows, reviewToken } = wizardState
    if (selectedAccountId === null || reviewToken === null) return

    setWizardState((prev) => ({ ...prev, importing: true }))
    try {
      const { imported } = await applyCsvImport({
        accountId: selectedAccountId,
        rows: importRows.map(toIncomingRow),
        choices: importRows.map(toChoice),
        reviewToken,
      })

      const { columnMapping, rules, loadedProfile, originalProfileColumnMapping, originalProfileRules } = wizardState
      const profileChanged = JSON.stringify(columnMapping) !== JSON.stringify(originalProfileColumnMapping) || JSON.stringify(rules) !== JSON.stringify(originalProfileRules)

      if (loadedProfile !== null) {
        setPreferredProfile(selectedAccountId, loadedProfile.id).catch(() => {})
      }

      await onSuccess()
      if (loadedProfile === null || profileChanged) {
        setWizardState((prev) => ({ ...prev, step: 'save-profile', importedCount: imported }))
      } else {
        if (imported === 0) toast.info(t('import.nothingNew'))
        else toast.success(t('import.success', { count: imported }))
        onClose()
      }
    } catch (err) {
      if (hasErrorCode(err, 'IMPORT_CHANGED')) {
        toast.warning(t('import.reviewStep.changedDuringReview'))
        await previewAgain(importRows)
      } else {
        toast.error(t('errors.importCashflows', { error: extractErrorMessage(err) }))
      }
    } finally {
      setWizardState((prev) => ({ ...prev, importing: false }))
    }
  }, [wizardState, previewAgain, t, onSuccess, onClose])

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
        onClose()
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    },
    [wizardState, t, onClose],
  )

  const handleUpdateProfile = useCallback(async () => {
    const { columnMapping, rules, loadedProfile } = wizardState
    if (loadedProfile === null) return
    const columnMappingJson = JSON.stringify(columnMapping)
    const ruleInputs = rules.map((rule) => {
      const { type, sortOrder, ...params } = rule
      return { ruleType: type, sortOrder, paramsJson: JSON.stringify(params) }
    })
    try {
      await updateImportProfile(loadedProfile.id, loadedProfile.name, columnMappingJson, ruleInputs)
      onClose()
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }, [wizardState, onClose])

  const splitValidationErrors = useMemo(
    () =>
      wizardState.importRows
        .filter((r) => r.splitLegs !== null && r.isSelected)
        .flatMap((r) => {
          const date = r.date.substring(0, 10)
          if (r.splitLegs!.length < 2) return [t('import.reviewStep.split.minLegsError', { date })]
          if (r.splitLegs!.reduce((sum, l) => sum + l.amountMinor, 0) !== r.amountMinor) return [t('import.reviewStep.split.validationError', { date })]
          return []
        }),
    [wizardState.importRows, t],
  )

  const canApplyReview =
    wizardState.importRows.length > 0 && wizardState.reviewToken !== null && !wizardState.previewing && !wizardState.importing && splitValidationErrors.length === 0

  return {
    wizardState,
    ibanLookup,
    availableBuckets,
    availableProfiles,
    selectedCount,
    canApplyReview,
    balanceWarningDates: wizardState.balanceUpdateDays,
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
    goBack,
  }
}
