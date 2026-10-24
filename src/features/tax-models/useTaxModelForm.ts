import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import { createTaxModel, updateTaxModel, type BracketInput } from '../../shared/api'
import { toMinorUnits, fromMinorUnits, pctToBps, bpsToPct } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import type { PersonRow, TaxModelDetail } from '../../shared/types'
import type { BracketFormState, TierFormState } from './BracketEditor'

const currentYear = new Date().getFullYear()

interface FormState {
  name: string
  calendarYear: number
  personId: number | null
  vatStatus: 'none' | 'all_year' | 'from_date'
  vatFromDate: string
  reserveFundCurrentMinor: string
  reserveFundPctBps: string
  reserveFundMaxMinor: string
  dividendTaxRateBps: string
  brackets: BracketFormState[]
}

const defaultFormState = (): FormState => ({
  name: '',
  calendarYear: currentYear,
  personId: null,
  vatStatus: 'none',
  vatFromDate: '',
  reserveFundCurrentMinor: '',
  reserveFundPctBps: '',
  reserveFundMaxMinor: '',
  dividendTaxRateBps: '',
  brackets: [{ upperBoundMinor: '', rateType: 'flat', flatRateBps: '', tiers: [] }],
})

const formStateFromModel = (model: TaxModelDetail, minorUnits: number): FormState => {
  const parsedBrackets: BracketFormState[] = model.brackets.map((b, i, arr) => {
    let tiers: TierFormState[] = []
    if (b.rateType === 'progressive' && b.tiersJson) {
      try {
        const rawTiers = JSON.parse(b.tiersJson) as { thresholdMinor: number; rateBps: number }[]
        tiers = rawTiers.map((t) => ({
          thresholdMinor: fromMinorUnits(t.thresholdMinor, minorUnits),
          rateBps: bpsToPct(t.rateBps),
        }))
      } catch {
        tiers = []
      }
    }
    return {
      upperBoundMinor: i < arr.length - 1 ? fromMinorUnits(arr[i + 1].lowerBoundMinor, minorUnits) : '',
      rateType: b.rateType as 'flat' | 'progressive',
      flatRateBps: b.flatRateBps !== null ? bpsToPct(b.flatRateBps) : '',
      tiers,
    }
  })

  return {
    name: model.name,
    calendarYear: model.calendarYear,
    personId: model.personId,
    vatStatus: model.vatStatus as 'none' | 'all_year' | 'from_date',
    vatFromDate: model.vatFromDate ?? '',
    reserveFundCurrentMinor: model.reserveFundCurrentMinor !== null ? fromMinorUnits(model.reserveFundCurrentMinor, minorUnits) : '',
    reserveFundPctBps: model.reserveFundPctBps !== null ? bpsToPct(model.reserveFundPctBps) : '',
    reserveFundMaxMinor: model.reserveFundMaxMinor !== null ? fromMinorUnits(model.reserveFundMaxMinor, minorUnits) : '',
    dividendTaxRateBps: model.dividendTaxRateBps !== null ? bpsToPct(model.dividendTaxRateBps) : '',
    brackets: parsedBrackets.length > 0 ? parsedBrackets : [{ upperBoundMinor: '', rateType: 'flat', flatRateBps: '', tiers: [] }],
  }
}

export const useTaxModelForm = (modelId: number | undefined, persons: PersonRow[], initialData: TaxModelDetail | undefined) => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const { data: consolidationCurrency } = useConsolidationCurrencyQuery()
  const minorUnits = consolidationCurrency?.minorUnits ?? 2
  const currencyCode = consolidationCurrency?.code

  const [form, setForm] = useState<FormState>(() => (initialData ? formStateFromModel(initialData, minorUnits) : defaultFormState()))

  // The amount fields hold decimal strings, so they are re-derived when the model or the currency's minor units change.
  const [prevInitialData, setPrevInitialData] = useState(initialData)
  const [prevMinorUnits, setPrevMinorUnits] = useState(minorUnits)
  if (initialData !== prevInitialData || minorUnits !== prevMinorUnits) {
    setPrevInitialData(initialData)
    setPrevMinorUnits(minorUnits)
    if (initialData) {
      setForm(formStateFromModel(initialData, minorUnits))
    }
  }

  // In create mode the person defaults to the first legal one until the user picks another.
  const defaultPersonId = modelId === undefined ? ((persons.find((p) => p.personType === 'legal') ?? persons[0])?.id ?? null) : null
  const personId = form.personId ?? defaultPersonId

  const set =
    <K extends keyof FormState>(key: K) =>
    (value: FormState[K]) => {
      setForm((prev) => ({ ...prev, [key]: value }))
    }

  const selectedPersonType: string = (() => {
    if (personId === null) return 'physical'
    const found = persons.find((p) => p.id === personId)
    return found?.personType ?? 'physical'
  })()

  const handleSave = async (): Promise<void> => {
    if (form.brackets.length === 0) {
      toast.error(t('taxModels.validation.bracketRequired'))
      return
    }

    if (personId === null) {
      toast.error(t('taxModels.validation.personRequired'))
      return
    }

    const brackets: BracketInput[] = []
    let lowerBoundMinor = 0

    for (let i = 0; i < form.brackets.length; i++) {
      const bracket = form.brackets[i]
      const bracketNum = i + 1
      const isLast = i === form.brackets.length - 1
      let flatRateBps: number | null = null
      let tiersJson: string | null = null
      let upperBoundMinor = 0

      if (!bracket.rateType) {
        toast.error(t('taxModels.validation.rateTypeRequired', { bracket: bracketNum }))
        return
      }

      if (bracket.rateType === 'flat') {
        if (!bracket.flatRateBps) {
          toast.error(t('taxModels.validation.flatRateRequired', { bracket: bracketNum }))
          return
        }
        flatRateBps = pctToBps(bracket.flatRateBps)
        if (flatRateBps === null) {
          toast.error(t('taxModels.validation.flatRateInvalid', { bracket: bracketNum }))
          return
        }
      } else if (bracket.rateType === 'progressive') {
        if (bracket.tiers.length === 0) {
          toast.error(t('taxModels.validation.tierRequired', { bracket: bracketNum }))
          return
        }
        const tiers: { thresholdMinor: number; rateBps: number }[] = []
        for (let tierIndex = 0; tierIndex < bracket.tiers.length; tierIndex++) {
          const tier = bracket.tiers[tierIndex]
          if (!tier.thresholdMinor || !tier.rateBps) {
            toast.error(t('taxModels.validation.tierFieldsRequired', { bracket: bracketNum, tier: tierIndex + 1 }))
            return
          }
          const thresholdMinor = toMinorUnits(tier.thresholdMinor, minorUnits)
          const rateBps = pctToBps(tier.rateBps)
          if (Number.isNaN(thresholdMinor) || rateBps === null) {
            toast.error(t('taxModels.validation.tierFieldsInvalid', { bracket: bracketNum, tier: tierIndex + 1 }))
            return
          }
          tiers.push({ thresholdMinor, rateBps })
        }
        tiersJson = JSON.stringify(tiers)
      }

      if (!isLast && !bracket.upperBoundMinor) {
        toast.error(t('taxModels.validation.upperBoundRequired', { bracket: bracketNum }))
        return
      }

      if (isLast && bracket.upperBoundMinor) {
        toast.error(t('taxModels.validation.lastUpperBoundNotAllowed', { bracket: bracketNum }))
        return
      }

      if (!isLast) {
        upperBoundMinor = toMinorUnits(bracket.upperBoundMinor, minorUnits)
        if (Number.isNaN(upperBoundMinor)) {
          toast.error(t('taxModels.validation.upperBoundInvalid', { bracket: bracketNum }))
          return
        }
      }

      brackets.push({ sortOrder: i, lowerBoundMinor, rateType: bracket.rateType, flatRateBps, tiersJson })
      lowerBoundMinor = upperBoundMinor
    }

    if (selectedPersonType === 'legal') {
      const reservePctBps = pctToBps(form.reserveFundPctBps)
      if (reservePctBps !== null && reservePctBps > 0 && !form.reserveFundMaxMinor) {
        toast.error(t('taxModels.validation.reserveFundMaxRequired'))
        return
      }
    }

    try {
      const baseInput = {
        name: form.name,
        calendarYear: form.calendarYear,
        personId,
        vatStatus: form.vatStatus,
        vatFromDate: form.vatStatus === 'from_date' ? form.vatFromDate : null,
        reserveFundCurrentMinor: form.reserveFundCurrentMinor ? toMinorUnits(form.reserveFundCurrentMinor, minorUnits) : null,
        reserveFundPctBps: form.reserveFundPctBps ? (pctToBps(form.reserveFundPctBps) ?? null) : null,
        reserveFundMaxMinor: form.reserveFundMaxMinor ? toMinorUnits(form.reserveFundMaxMinor, minorUnits) : null,
        dividendTaxRateBps: form.dividendTaxRateBps ? (pctToBps(form.dividendTaxRateBps) ?? null) : null,
        brackets,
      }

      if (modelId !== undefined) {
        await updateTaxModel({ ...baseInput, modelId })
        queryClient.invalidateQueries({ queryKey: ['taxModels'] })
        queryClient.invalidateQueries({ queryKey: ['taxModel', modelId] })
        queryClient.invalidateQueries({ queryKey: ['taxModelResults', modelId] })
        navigate({ to: '/tax-models/$modelId/results', params: { modelId: String(modelId) } })
      } else {
        const newId = await createTaxModel(baseInput)
        queryClient.invalidateQueries({ queryKey: ['taxModels'] })
        navigate({ to: '/tax-models/$modelId/results', params: { modelId: String(newId) } })
      }
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  return {
    name: form.name,
    setName: set('name'),
    calendarYear: form.calendarYear,
    setCalendarYear: set('calendarYear'),
    personId,
    setPersonId: set('personId'),
    vatStatus: form.vatStatus,
    setVatStatus: set('vatStatus'),
    vatFromDate: form.vatFromDate,
    setVatFromDate: set('vatFromDate'),
    reserveFundCurrentMinor: form.reserveFundCurrentMinor,
    setReserveFundCurrentMinor: set('reserveFundCurrentMinor'),
    reserveFundPctBps: form.reserveFundPctBps,
    setReserveFundPctBps: set('reserveFundPctBps'),
    reserveFundMaxMinor: form.reserveFundMaxMinor,
    setReserveFundMaxMinor: set('reserveFundMaxMinor'),
    dividendTaxRateBps: form.dividendTaxRateBps,
    setDividendTaxRateBps: set('dividendTaxRateBps'),
    brackets: form.brackets,
    setBrackets: set('brackets'),
    selectedPersonType,
    minorUnits,
    currencyCode,
    handleSave,
  }
}
