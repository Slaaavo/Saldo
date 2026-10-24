import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { toast } from 'sonner'
import CreateAssetModal from './CreateAssetModal'
import type { Currency, PersonRow } from '../../../shared/types'

vi.mock('../../../shared/api', () => ({
  listCurrencies: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  listCustomUnits: vi.fn(),
  createCustomUnit: vi.fn(),
  createAccount: vi.fn(),
  listPersons: vi.fn(),
}))

vi.mock('../../../app/useSelectedPerson', () => ({
  useSelectedPerson: () => ({ selectedPersonId: null, setSelectedPersonId: vi.fn() }),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (!opts) return key
      let result = key
      for (const [k, v] of Object.entries(opts)) result = result.replace(`{{${k}}}`, String(v))
      return result
    },
    i18n: { language: 'en' },
  }),
}))

vi.mock('sonner', () => ({ toast: { error: vi.fn(), success: vi.fn() } }))

vi.mock('../../../shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

vi.mock('../../currency/CurrencySelect', () => ({
  default: ({ currencies, value, onChange }: { currencies: Currency[]; value: Currency | null; onChange: (c: Currency) => void }) => (
    <select data-testid="currency-select" value={value?.id ?? ''} onChange={(e) => onChange(currencies.find((c) => String(c.id) === e.target.value)!)}>
      {currencies.map((c) => (
        <option key={c.id} value={c.id}>
          {c.code}
        </option>
      ))}
    </select>
  ),
}))

import { listCurrencies, getConsolidationCurrency, listCustomUnits, createCustomUnit, createAccount, listPersons } from '../../../shared/api'

const EUR: Currency = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }
const USD: Currency = { id: 2, code: 'USD', name: 'US Dollar', minorUnits: 2, isCustom: false }

const makePerson = (id: number, personType: string): PersonRow => ({
  id,
  name: `Person ${id}`,
  personType,
  isDefault: true,
  createdAt: '2025-01-01T00:00:00',
  defaultRevenueAccountId: 1,
  defaultExpenseAccountId: 2,
  vatPayer: false,
})

const setupMocks = (personType: 'legal' | 'physical' | null = 'legal') => {
  ;(listCurrencies as Mock).mockResolvedValue([EUR, USD])
  ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
  ;(listCustomUnits as Mock).mockResolvedValue([])
  ;(listPersons as Mock).mockResolvedValue(personType ? [makePerson(5, personType)] : [])
  ;(createCustomUnit as Mock).mockResolvedValue(77)
  ;(createAccount as Mock).mockResolvedValue(100)
}

const renderModal = async () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const user = userEvent.setup()
  render(
    <QueryClientProvider client={queryClient}>
      <CreateAssetModal onSuccess={vi.fn()} onClose={vi.fn()} />
    </QueryClientProvider>,
  )
  await waitFor(() => expect(screen.getByTestId('currency-select')).toHaveValue('1'))
  return user
}

const NAME_LABEL = 'modals.createAsset.nameLabel'
const SUBMIT = 'modals.createAccount.submit'

describe('CreateAssetModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('depreciation fields', () => {
    it('sends the purchase price, date and period for a legal person in the consolidation currency', async () => {
      setupMocks('legal')
      const user = await renderModal()

      await user.type(screen.getByLabelText(NAME_LABEL), 'Laptop')
      await user.type(await screen.findByLabelText('modals.assetDepreciation.purchasePrice'), '1000')
      await user.type(screen.getByLabelText('modals.assetDepreciation.depreciationPeriodMonths'), '48')
      await user.click(screen.getByText(SUBMIT))

      await waitFor(() => expect(createAccount).toHaveBeenCalledOnce())
      const args = (createAccount as Mock).mock.calls[0]
      expect(args.slice(8)).toEqual([100000, expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/), 48])
    })

    it('sends no depreciation values when no purchase price is entered', async () => {
      setupMocks('legal')
      const user = await renderModal()

      await user.type(screen.getByLabelText(NAME_LABEL), 'Laptop')
      await user.click(screen.getByText(SUBMIT))

      await waitFor(() => expect(createAccount).toHaveBeenCalledOnce())
      expect((createAccount as Mock).mock.calls[0].slice(8)).toEqual([null, null, null])
    })

    it('hides the depreciation fields and sends nothing for a physical person', async () => {
      setupMocks('physical')
      const user = await renderModal()

      expect(screen.queryByLabelText('modals.assetDepreciation.purchasePrice')).not.toBeInTheDocument()
      await user.type(screen.getByLabelText(NAME_LABEL), 'Bike')
      await user.click(screen.getByText(SUBMIT))

      await waitFor(() => expect(createAccount).toHaveBeenCalledOnce())
      expect((createAccount as Mock).mock.calls[0].slice(8)).toEqual([null, null, null])
    })

    it('hides the depreciation fields and drops typed values when the currency is not the consolidation currency', async () => {
      setupMocks('legal')
      const user = await renderModal()

      await user.type(screen.getByLabelText(NAME_LABEL), 'US laptop')
      await user.type(await screen.findByLabelText('modals.assetDepreciation.purchasePrice'), '1000')
      await user.selectOptions(screen.getByTestId('currency-select'), 'USD')

      expect(screen.queryByLabelText('modals.assetDepreciation.purchasePrice')).not.toBeInTheDocument()
      await user.click(screen.getByText(SUBMIT))

      await waitFor(() => expect(createAccount).toHaveBeenCalledOnce())
      expect((createAccount as Mock).mock.calls[0].slice(8)).toEqual([null, null, null])
    })
  })

  describe('creating a unit asset', () => {
    const fillUnitAsset = async (user: Awaited<ReturnType<typeof renderModal>>, price: string) => {
      await user.click(screen.getByText('modals.createAsset.denominationUnit'))
      await user.type(screen.getByLabelText(NAME_LABEL), 'ETF')
      await user.type(screen.getByLabelText('modals.createAsset.unitName'), 'VWCE')
      await user.type(screen.getByLabelText('modals.createAsset.quantity'), '2')
      await user.type(screen.getByLabelText('modals.createAsset.pricePerUnit'), price)
    }

    it('does not create the unit when the price is not positive', async () => {
      setupMocks('legal')
      const user = await renderModal()

      await fillUnitAsset(user, '0')
      await user.click(screen.getByText(SUBMIT))

      expect(toast.error).toHaveBeenCalledWith('units.invalidPrice')
      expect(createCustomUnit).not.toHaveBeenCalled()
      expect(createAccount).not.toHaveBeenCalled()
    })

    it('does not create the unit when no person is available', async () => {
      setupMocks(null)
      const user = await renderModal()

      await fillUnitAsset(user, '10')
      await user.click(screen.getByText(SUBMIT))

      expect(toast.error).toHaveBeenCalledWith('modals.createAsset.personRequired')
      expect(createCustomUnit).not.toHaveBeenCalled()
      expect(createAccount).not.toHaveBeenCalled()
    })

    it('reuses the new unit when the account creation is retried after a failure', async () => {
      setupMocks('legal')
      ;(createAccount as Mock).mockRejectedValueOnce({ code: 'DB', message: 'boom' }).mockResolvedValueOnce(100)
      const user = await renderModal()

      await fillUnitAsset(user, '10')
      await user.click(screen.getByText(SUBMIT))
      await waitFor(() => expect(toast.error).toHaveBeenCalledWith('errors.createAccount'))

      await user.click(screen.getByText(SUBMIT))

      await waitFor(() => expect(createAccount).toHaveBeenCalledTimes(2))
      expect(createCustomUnit).toHaveBeenCalledOnce()
      expect((createAccount as Mock).mock.calls[1][1]).toBe(77)
    })
  })

  it('toasts when the initial data cannot be loaded', async () => {
    setupMocks('legal')
    ;(listCurrencies as Mock).mockRejectedValue({ code: 'DB', message: 'offline' })
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
    render(
      <QueryClientProvider client={queryClient}>
        <CreateAssetModal onSuccess={vi.fn()} onClose={vi.fn()} />
      </QueryClientProvider>,
    )

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('offline'))
  })
})
