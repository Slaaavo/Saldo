import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import EditAccountModal from './EditAccountModal'

vi.mock('../../shared/api', () => ({
  listPersons: vi.fn(),
  getConsolidationCurrency: vi.fn(),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}))

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: ({ id, value, onChange }: { id?: string; value: string; onChange: (v: string) => void }) => (
    <input id={id} value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

vi.mock('sonner', () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}))

import { listPersons, getConsolidationCurrency } from '../../shared/api'

const makeWrapper = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const Wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  return Wrapper
}

const renderModal = (overrides: { currencyCode: string; currentPurchasePriceMinor?: number | null }) =>
  render(
    <EditAccountModal
      accountId={10}
      currentName="Car"
      accountType="asset"
      currentPersonId={2}
      isCustomUnit={false}
      currencyCode={overrides.currencyCode}
      currencyMinorUnits={2}
      currentPurchasePriceMinor={overrides.currentPurchasePriceMinor ?? null}
      currentPurchaseDate={null}
      currentDepreciationPeriodMonths={null}
      onSubmit={vi.fn()}
      onClose={vi.fn()}
    />,
    { wrapper: makeWrapper() },
  )

describe('EditAccountModal depreciation fields', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listPersons as Mock).mockResolvedValue([
      { id: 1, name: 'Personal', personType: 'physical', isDefault: true, vatPayer: false },
      { id: 2, name: 'Company', personType: 'legal', isDefault: false, vatPayer: true },
    ])
    ;(getConsolidationCurrency as Mock).mockResolvedValue({ id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false })
  })

  it('shows them for a legal person asset in the consolidation currency', async () => {
    renderModal({ currencyCode: 'EUR' })

    await waitFor(() => expect(screen.getByLabelText('modals.assetDepreciation.purchasePrice')).toBeInTheDocument())
  })

  it('hides them for an asset in another currency', async () => {
    renderModal({ currencyCode: 'USD' })

    await waitFor(() => expect(screen.getAllByText('Company').length).toBeGreaterThan(0))
    await waitFor(() => expect(getConsolidationCurrency).toHaveBeenCalled())
    expect(screen.queryByLabelText('modals.assetDepreciation.purchasePrice')).not.toBeInTheDocument()
  })

  it('keeps them visible for an asset in another currency that already has depreciation data', async () => {
    renderModal({ currencyCode: 'USD', currentPurchasePriceMinor: 100_000 })

    await waitFor(() => expect(screen.getByLabelText('modals.assetDepreciation.purchasePrice')).toBeInTheDocument())
  })
})
