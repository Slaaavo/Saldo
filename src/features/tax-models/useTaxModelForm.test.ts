import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { createElement } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import { useTaxModelForm } from './useTaxModelForm'
import type { Currency, PersonRow, TaxModelDetail } from '../../shared/types'
import type { BracketFormState } from './BracketEditor'
import { makeFigures } from './taxForecastFixtures'

vi.mock('../../shared/api', () => ({
  createTaxModel: vi.fn(),
  updateTaxModel: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const navigateMock = vi.fn()
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => navigateMock }))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (!opts) return key
      let result = key
      for (const [k, v] of Object.entries(opts)) result = result.replace(`{{${k}}}`, String(v))
      return result
    },
  }),
}))

import { createTaxModel, updateTaxModel, getConsolidationCurrency } from '../../shared/api'

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const JPY: Currency = { id: 2, code: 'JPY', name: 'Japanese Yen', minorUnits: 0, isCustom: false }

const makePerson = (overrides: Partial<PersonRow>): PersonRow => ({
  id: 1,
  name: 'Person',
  personType: 'physical',
  isDefault: false,
  createdAt: '2025-01-01T00:00:00',
  defaultRevenueAccountId: 1,
  defaultExpenseAccountId: 2,
  vatPayer: false,
  ...overrides,
})

const PHYSICAL = makePerson({ id: 1, name: 'Alice', personType: 'physical' })
const LEGAL = makePerson({ id: 2, name: 'Acme s.r.o.', personType: 'legal' })

const flatBracket = (overrides?: Partial<BracketFormState>): BracketFormState => ({
  upperBoundMinor: '',
  rateType: 'flat',
  flatRateBps: '19',
  tiers: [],
  ...overrides,
})

const makeModel = (overrides?: Partial<TaxModelDetail>): TaxModelDetail => ({
  id: 7,
  name: 'Existing',
  calendarYear: 2025,
  personId: 1,
  personName: 'Alice',
  personType: 'physical',
  vatStatus: 'none',
  vatFromDate: null,
  reserveFundCurrentMinor: null,
  reserveFundPctBps: null,
  reserveFundMaxMinor: null,
  dividendTaxRateBps: null,
  createdAt: '2025-01-01T00:00:00',
  updatedAt: '2025-01-01T00:00:00',
  brackets: [
    { id: 1, sortOrder: 0, lowerBoundMinor: 0, rateType: 'flat', flatRateBps: 1500, tiersJson: null },
    { id: 2, sortOrder: 1, lowerBoundMinor: 4000000, rateType: 'flat', flatRateBps: 2500, tiersJson: null },
  ],
  minimumTaxBands: [],
  figures: makeFigures(),
  ...overrides,
})

const makeQueryClient = () => new QueryClient({ defaultOptions: { queries: { retry: 0 } } })

const makeWrapper = (queryClient: QueryClient) => {
  const Wrapper = ({ children }: { children: Parameters<typeof createElement>[2] }) => createElement(QueryClientProvider, { client: queryClient }, children)
  return Wrapper
}

describe('useTaxModelForm', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(createTaxModel as Mock).mockResolvedValue(11)
    ;(updateTaxModel as Mock).mockResolvedValue(undefined)
    ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
  })

  describe('default person', () => {
    it('selects the first legal person when persons are already loaded on first render', () => {
      const persons = [PHYSICAL, LEGAL]
      const { result } = renderHook(() => useTaxModelForm(undefined, persons, undefined), { wrapper: makeWrapper(makeQueryClient()) })

      expect(result.current.personId).toBe(LEGAL.id)
      expect(result.current.selectedPersonType).toBe('legal')
    })

    it('selects the default person once persons arrive after the first render', () => {
      const { result, rerender } = renderHook(({ persons }) => useTaxModelForm(undefined, persons, undefined), {
        wrapper: makeWrapper(makeQueryClient()),
        initialProps: { persons: [] as PersonRow[] },
      })
      expect(result.current.personId).toBeNull()

      rerender({ persons: [PHYSICAL, LEGAL] })

      expect(result.current.personId).toBe(LEGAL.id)
    })

    it('renders without looping while persons is a fresh empty array on every render', () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [], undefined), { wrapper: makeWrapper(makeQueryClient()) })

      expect(result.current.personId).toBeNull()
      expect(result.current.selectedPersonType).toBe('physical')
    })

    it('falls back to the first person when none is legal', () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [PHYSICAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })

      expect(result.current.personId).toBe(PHYSICAL.id)
    })

    it('keeps an explicit pick over the default', () => {
      const persons = [PHYSICAL, LEGAL]
      const { result } = renderHook(() => useTaxModelForm(undefined, persons, undefined), { wrapper: makeWrapper(makeQueryClient()) })

      act(() => result.current.setPersonId(PHYSICAL.id))

      expect(result.current.personId).toBe(PHYSICAL.id)
    })

    it('does not default the person in edit mode', () => {
      const persons = [PHYSICAL, LEGAL]
      const { result } = renderHook(() => useTaxModelForm(7, persons, undefined), { wrapper: makeWrapper(makeQueryClient()) })

      expect(result.current.personId).toBeNull()
    })
  })

  describe('consolidation currency without decimals', () => {
    const makeJpyClient = () => {
      const queryClient = makeQueryClient()
      queryClient.setQueryData(['consolidation-currency'], JPY)
      return queryClient
    }

    it('exposes the consolidation currency code and minor units', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [PHYSICAL], undefined), { wrapper: makeWrapper(makeJpyClient()) })

      expect(result.current.currencyCode).toBe('JPY')
      expect(result.current.minorUnits).toBe(0)
    })

    it('saves amounts as whole units: 1000 JPY stays 1000', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeJpyClient()) })
      act(() => {
        result.current.setBrackets([
          flatBracket({ upperBoundMinor: '1000', flatRateBps: '15' }),
          { upperBoundMinor: '', rateType: 'progressive', flatRateBps: '', tiers: [{ thresholdMinor: '250', rateBps: '20' }] },
        ])
        result.current.setReserveFundCurrentMinor('300')
        result.current.setReserveFundPctBps('10')
        result.current.setReserveFundMaxMinor('5000')
      })

      await act(() => result.current.handleSave())

      const input = (createTaxModel as Mock).mock.calls[0][0]
      expect(input.brackets[1].lowerBoundMinor).toBe(1000)
      expect(JSON.parse(input.brackets[1].tiersJson)).toEqual([{ thresholdMinor: 250, rateBps: 2000 }])
      expect(input.reserveFundCurrentMinor).toBe(300)
      expect(input.reserveFundMaxMinor).toBe(5000)
    })

    it('loads an existing model without moving the decimal point', () => {
      const model = makeModel({
        reserveFundCurrentMinor: 700,
        reserveFundMaxMinor: 9000,
        brackets: [
          { id: 1, sortOrder: 0, lowerBoundMinor: 0, rateType: 'flat', flatRateBps: 1500, tiersJson: null },
          { id: 2, sortOrder: 1, lowerBoundMinor: 1000, rateType: 'progressive', flatRateBps: null, tiersJson: JSON.stringify([{ thresholdMinor: 250, rateBps: 2000 }]) },
        ],
      })
      const { result } = renderHook(() => useTaxModelForm(7, [PHYSICAL], model), { wrapper: makeWrapper(makeJpyClient()) })

      expect(result.current.brackets[0].upperBoundMinor).toBe('1000')
      expect(result.current.brackets[1].tiers[0].thresholdMinor).toBe('250')
      expect(result.current.reserveFundCurrentMinor).toBe('700')
      expect(result.current.reserveFundMaxMinor).toBe('9000')
    })

    it('reloads an existing model in the right units when the currency arrives after the model', async () => {
      ;(getConsolidationCurrency as Mock).mockResolvedValue(JPY)
      const model = makeModel()
      const { result } = renderHook(() => useTaxModelForm(7, [PHYSICAL], model), { wrapper: makeWrapper(makeQueryClient()) })

      await waitFor(() => expect(result.current.brackets[0].upperBoundMinor).toBe('4000000'))
    })
  })

  describe('saving', () => {
    it('maps each bracket lower bound to the previous bracket upper bound', async () => {
      const persons = [PHYSICAL]
      const { result } = renderHook(() => useTaxModelForm(undefined, persons, undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => {
        result.current.setName('Model')
        result.current.setBrackets([
          flatBracket({ upperBoundMinor: '40000.50', flatRateBps: '15' }),
          flatBracket({ upperBoundMinor: '90000', flatRateBps: '25' }),
          flatBracket({ flatRateBps: '35' }),
        ])
      })

      await act(() => result.current.handleSave())

      const input = (createTaxModel as Mock).mock.calls[0][0]
      expect(input.brackets.map((b: { lowerBoundMinor: number }) => b.lowerBoundMinor)).toEqual([0, 4000050, 9000000])
      expect(input.brackets.map((b: { flatRateBps: number }) => b.flatRateBps)).toEqual([1500, 2500, 3500])
    })

    it('round-trips an existing model through the form without moving bracket bounds', async () => {
      const persons = [PHYSICAL]
      const { result } = renderHook(() => useTaxModelForm(7, persons, makeModel()), { wrapper: makeWrapper(makeQueryClient()) })

      expect(result.current.brackets.map((b) => b.upperBoundMinor)).toEqual(['40000.00', ''])

      await act(() => result.current.handleSave())

      const input = (updateTaxModel as Mock).mock.calls[0][0]
      expect(input.modelId).toBe(7)
      expect(input.brackets.map((b: { lowerBoundMinor: number }) => b.lowerBoundMinor)).toEqual([0, 4000000])
    })

    it('invalidates the cached tax model results after updating', async () => {
      const queryClient = makeQueryClient()
      const invalidate = vi.spyOn(queryClient, 'invalidateQueries')
      const { result } = renderHook(() => useTaxModelForm(7, [PHYSICAL], makeModel()), { wrapper: makeWrapper(queryClient) })

      await act(() => result.current.handleSave())

      expect(invalidate).toHaveBeenCalledWith({ queryKey: ['taxModelResults', 7] })
      expect(navigateMock).toHaveBeenCalledWith({ to: '/tax-models/$modelId/results', params: { modelId: '7' } })
    })

    it('rejects a reserve fund allocation percentage without a maximum for a legal person', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => {
        result.current.setBrackets([flatBracket()])
        result.current.setReserveFundPctBps('10')
      })

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.validation.reserveFundMaxRequired')
      expect(createTaxModel).not.toHaveBeenCalled()
    })

    it('saves a reserve fund allocation when a maximum is set', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => {
        result.current.setBrackets([flatBracket()])
        result.current.setReserveFundPctBps('10')
        result.current.setReserveFundMaxMinor('5000')
      })

      await act(() => result.current.handleSave())

      const input = (createTaxModel as Mock).mock.calls[0][0]
      expect(input.reserveFundPctBps).toBe(1000)
      expect(input.reserveFundMaxMinor).toBe(500000)
    })

    it('allows an empty reserve fund maximum when there is no allocation percentage', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => result.current.setBrackets([flatBracket()]))

      await act(() => result.current.handleSave())

      expect(createTaxModel).toHaveBeenCalledOnce()
    })

    it('reports an invalid bracket upper bound instead of saving it as 0', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [PHYSICAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => result.current.setBrackets([flatBracket({ upperBoundMinor: 'abc' }), flatBracket()]))

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.validation.upperBoundInvalid')
      expect(createTaxModel).not.toHaveBeenCalled()
    })

    it('reports an invalid tier threshold instead of saving it as NaN', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [PHYSICAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => result.current.setBrackets([{ upperBoundMinor: '', rateType: 'progressive', flatRateBps: '', tiers: [{ thresholdMinor: 'x', rateBps: '10' }] }]))

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.validation.tierFieldsInvalid')
      expect(createTaxModel).not.toHaveBeenCalled()
    })

    it('serialises valid progressive tiers', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [PHYSICAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() =>
        result.current.setBrackets([
          {
            upperBoundMinor: '',
            rateType: 'progressive',
            flatRateBps: '',
            tiers: [
              { thresholdMinor: '0', rateBps: '10' },
              { thresholdMinor: '100.5', rateBps: '20' },
            ],
          },
        ]),
      )

      await act(() => result.current.handleSave())

      const input = (createTaxModel as Mock).mock.calls[0][0]
      expect(JSON.parse(input.brackets[0].tiersJson)).toEqual([
        { thresholdMinor: 0, rateBps: 1000 },
        { thresholdMinor: 10050, rateBps: 2000 },
      ])
    })

    it('saves the minimum tax bands of a company with each lower bound taken from the band before', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => {
        result.current.setBrackets([flatBracket()])
        result.current.setMinimumTaxBands([
          { upperBoundMinor: '50000', amountMinor: '340' },
          { upperBoundMinor: '250000', amountMinor: '960.50' },
          { upperBoundMinor: '', amountMinor: '1920' },
        ])
      })

      await act(() => result.current.handleSave())

      const input = (createTaxModel as Mock).mock.calls[0][0]
      expect(input.minimumTaxBands).toEqual([
        { lowerBoundMinor: 0, amountMinor: 34000 },
        { lowerBoundMinor: 5000000, amountMinor: 96050 },
        { lowerBoundMinor: 25000000, amountMinor: 192000 },
      ])
    })

    it('loads the minimum tax bands of an existing model as revenue limits and saves them back unchanged', async () => {
      const bands = [
        { lowerBoundMinor: 0, amountMinor: 34000 },
        { lowerBoundMinor: 5000000, amountMinor: 96000 },
      ]
      const model = makeModel({ personId: 2, personName: 'Acme s.r.o.', personType: 'legal', minimumTaxBands: bands })
      const { result } = renderHook(() => useTaxModelForm(7, [LEGAL], model), { wrapper: makeWrapper(makeQueryClient()) })

      expect(result.current.minimumTaxBands).toEqual([
        { upperBoundMinor: '50000.00', amountMinor: '340.00' },
        { upperBoundMinor: '', amountMinor: '960.00' },
      ])

      await act(() => result.current.handleSave())

      expect((updateTaxModel as Mock).mock.calls[0][0].minimumTaxBands).toEqual(bands)
    })

    it('ignores a hidden, half-filled band row when saving for a physical person', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [PHYSICAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => {
        result.current.setBrackets([flatBracket()])
        result.current.setMinimumTaxBands([{ upperBoundMinor: '', amountMinor: '' }])
      })

      await act(() => result.current.handleSave())

      expect(toast.error).not.toHaveBeenCalled()
      expect((createTaxModel as Mock).mock.calls[0][0].minimumTaxBands).toEqual([])
    })

    it('saves the bands of an existing company model back unchanged after it is edited under a physical person', async () => {
      const bands = [
        { lowerBoundMinor: 0, amountMinor: 34000 },
        { lowerBoundMinor: 5000000, amountMinor: 96000 },
      ]
      const model = makeModel({ personId: 2, personName: 'Acme s.r.o.', personType: 'legal', minimumTaxBands: bands })
      const { result } = renderHook(() => useTaxModelForm(7, [LEGAL, PHYSICAL], model), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => result.current.setPersonId(PHYSICAL.id))

      await act(() => result.current.handleSave())

      expect(result.current.selectedPersonType).toBe('physical')
      expect((updateTaxModel as Mock).mock.calls[0][0].minimumTaxBands).toEqual(bands)
    })

    it('saves no minimum tax bands when a company has none', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => result.current.setBrackets([flatBracket()]))

      await act(() => result.current.handleSave())

      expect((createTaxModel as Mock).mock.calls[0][0].minimumTaxBands).toEqual([])
    })

    it('reports the band whose revenue limit does not rise instead of saving it', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [LEGAL], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => {
        result.current.setBrackets([flatBracket()])
        result.current.setMinimumTaxBands([
          { upperBoundMinor: '1000', amountMinor: '340' },
          { upperBoundMinor: '900', amountMinor: '960' },
          { upperBoundMinor: '', amountMinor: '1920' },
        ])
      })

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.validation.minimumTax.upperBoundNotIncreasing')
      expect(createTaxModel).not.toHaveBeenCalled()
    })

    it('requires a person to be selected', async () => {
      const { result } = renderHook(() => useTaxModelForm(undefined, [], undefined), { wrapper: makeWrapper(makeQueryClient()) })
      act(() => result.current.setBrackets([flatBracket()]))

      await act(() => result.current.handleSave())

      expect(toast.error).toHaveBeenCalledWith('taxModels.validation.personRequired')
      expect(createTaxModel).not.toHaveBeenCalled()
    })
  })
})
