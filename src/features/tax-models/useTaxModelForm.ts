import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'
import { createTaxModel, updateTaxModel } from '../../shared/api'
import { toMinorUnits, fromMinorUnits, pctToBps, bpsToPct } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
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

const formStateFromModel = (model: TaxModelDetail): FormState => {
  const parsedBrackets: BracketFormState[] = model.brackets.map((b, i, arr) => {
    let tiers: TierFormState[] = []
    if (b.rateType === 'progressive' && b.tiersJson) {
      try {
        const rawTiers = JSON.parse(b.tiersJson) as { thresholdMinor: number; rateBps: number }[]
        tiers = rawTiers.map((t) => ({
          thresholdMinor: fromMinorUnits(t.thresholdMinor, 2),
          rateBps: bpsToPct(t.rateBps),
        }))
      } catch {
        tiers = []
      }
    }
    return {
      upperBoundMinor: i < arr.length - 1 ? fromMinorUnits(arr[i + 1].lowerBoundMinor, 2) : '',
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
    reserveFundCurrentMinor: model.reserveFundCurrentMinor !== null ? fromMinorUnits(model.reserveFundCurrentMinor, 2) : '',
    reserveFundPctBps: model.reserveFundPctBps !== null ? bpsToPct(model.reserveFundPctBps) : '',
    reserveFundMaxMinor: model.reserveFundMaxMinor !== null ? fromMinorUnits(model.reserveFundMaxMinor, 2) : '',
    dividendTaxRateBps: model.dividendTaxRateBps !== null ? bpsToPct(model.dividendTaxRateBps) : '',
    brackets: parsedBrackets.length > 0 ? parsedBrackets : [{ upperBoundMinor: '', rateType: 'flat', flatRateBps: '', tiers: [] }],
  }
}

export const useTaxModelForm = (modelId: number | undefined, persons: PersonRow[], initialData: TaxModelDetail | undefined) => {
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const [form, setForm] = useState<FormState>(() => (initialData ? formStateFromModel(initialData) : defaultFormState()))

  const [prevInitialData, setPrevInitialData] = useState(initialData)
  if (initialData !== prevInitialData) {
    setPrevInitialData(initialData)
    if (initialData) {
      setForm(formStateFromModel(initialData))
    }
  }

  // Auto-select first legal person in create mode when persons load
  const [prevPersons, setPrevPersons] = useState(persons)
  if (persons !== prevPersons) {
    setPrevPersons(persons)
    if (modelId === undefined && persons.length > 0) {
      setForm((prev) => {
        if (prev.personId !== null) return prev
        const firstLegal = persons.find((p) => p.personType === 'legal')
        const defaultPerson = firstLegal ?? persons[0]
        return { ...prev, personId: defaultPerson.id }
      })
    }
  }

  const set =
    <K extends keyof FormState>(key: K) =>
    (value: FormState[K]) => {
      setForm((prev) => ({ ...prev, [key]: value }))
    }

  const selectedPersonType: string = (() => {
    if (form.personId === null) return 'physical'
    const found = persons.find((p) => p.id === form.personId)
    return found?.personType ?? 'physical'
  })()

  const handleSave = async (): Promise<void> => {
    if (form.brackets.length === 0) {
      toast.error('At least one bracket is required')
      return
    }

    if (form.personId === null) {
      toast.error('A person must be selected')
      return
    }

    for (let i = 0; i < form.brackets.length; i++) {
      const bracket = form.brackets[i]
      const bracketNum = i + 1
      const isLast = i === form.brackets.length - 1

      if (!bracket.rateType) {
        toast.error(`Bracket ${bracketNum}: rate type is required`)
        return
      }

      if (bracket.rateType === 'flat') {
        if (!bracket.flatRateBps) {
          toast.error(`Bracket ${bracketNum}: flat rate is required`)
          return
        }
      } else if (bracket.rateType === 'progressive') {
        if (bracket.tiers.length === 0) {
          toast.error(`Bracket ${bracketNum}: at least one tier is required`)
          return
        }
        for (let t = 0; t < bracket.tiers.length; t++) {
          const tier = bracket.tiers[t]
          if (!tier.thresholdMinor || !tier.rateBps) {
            toast.error(`Bracket ${bracketNum}, tier ${t + 1}: threshold and rate are required`)
            return
          }
        }
      }

      if (!isLast && !bracket.upperBoundMinor) {
        toast.error(`Bracket ${bracketNum}: upper bound is required`)
        return
      }

      if (isLast && bracket.upperBoundMinor) {
        toast.error(`Bracket ${bracketNum}: the last bracket must not have an upper bound set`)
        return
      }
    }

    try {
      const buildBrackets = form.brackets.map((b, i) => ({
        sortOrder: i,
        lowerBoundMinor: i === 0 ? 0 : toMinorUnits(form.brackets[i - 1].upperBoundMinor, 2) || 0,
        rateType: b.rateType,
        flatRateBps: b.rateType === 'flat' ? (pctToBps(b.flatRateBps) ?? null) : null,
        tiersJson:
          b.rateType === 'progressive'
            ? JSON.stringify(
                b.tiers.map((t) => ({
                  thresholdMinor: toMinorUnits(t.thresholdMinor, 2) ?? 0,
                  rateBps: pctToBps(t.rateBps) ?? 0,
                })),
              )
            : null,
      }))

      const baseInput = {
        name: form.name,
        calendarYear: form.calendarYear,
        personId: form.personId,
        vatStatus: form.vatStatus,
        vatFromDate: form.vatStatus === 'from_date' ? form.vatFromDate : null,
        reserveFundCurrentMinor: form.reserveFundCurrentMinor ? toMinorUnits(form.reserveFundCurrentMinor, 2) : null,
        reserveFundPctBps: form.reserveFundPctBps ? (pctToBps(form.reserveFundPctBps) ?? null) : null,
        reserveFundMaxMinor: form.reserveFundMaxMinor ? toMinorUnits(form.reserveFundMaxMinor, 2) : null,
        dividendTaxRateBps: form.dividendTaxRateBps ? (pctToBps(form.dividendTaxRateBps) ?? null) : null,
        brackets: buildBrackets,
      }

      if (modelId !== undefined) {
        await updateTaxModel({ ...baseInput, modelId })
        queryClient.invalidateQueries({ queryKey: ['taxModels'] })
        queryClient.invalidateQueries({ queryKey: ['taxModel', modelId] })
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
    personId: form.personId,
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
    handleSave,
  }
}
