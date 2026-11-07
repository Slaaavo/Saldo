import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import EditAccountModal from './EditAccountModal'

vi.mock('../../shared/api', () => ({
  listPersons: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  getAssetUse: vi.fn(),
  previewAssetPurchase: vi.fn(),
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

import { toast } from 'sonner'
import { listPersons, getConsolidationCurrency, getAssetUse, previewAssetPurchase } from '../../shared/api'

const makeWrapper = () => {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const Wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  return Wrapper
}

const renderModal = (overrides: {
  currencyCode: string
  currentPurchasePriceMinor?: number | null
  currentPurchaseDate?: string | null
  currentDepreciationPeriodMonths?: number | null
  onSubmit?: Mock
}) =>
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
      currentPurchaseDate={overrides.currentPurchaseDate ?? null}
      currentDepreciationPeriodMonths={overrides.currentDepreciationPeriodMonths ?? null}
      onSubmit={overrides.onSubmit ?? vi.fn()}
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
    ;(getAssetUse as Mock).mockResolvedValue({ purchaseVat: null, businessUsePctBps: null, businessUseChanges: [] })
    ;(previewAssetPurchase as Mock).mockResolvedValue({
      depreciatingMinor: 1268292,
      reclaimedVatMinor: 145854,
      lostVatEntryMinor: 145854,
      startValueMinor: 1560000,
      monthlyMinor: 52846,
      monthlyDeductibleMinor: 42276,
    })
  })

  const saved = {
    currencyCode: 'EUR',
    currentPurchasePriceMinor: 1268292,
    currentPurchaseDate: '2026-07-12',
    currentDepreciationPeriodMonths: 24,
  }

  it('sends back the terms it read, so a rename keeps the shares', async () => {
    const terms = {
      purchaseVat: { grossMinor: 1560000, vatRateBps: 2300, vatReclaimablePctBps: 5000 },
      businessUsePctBps: 8000,
      businessUseChanges: [{ fromMonth: '2027-01', businessUsePctBps: 6000 }],
    }
    ;(getAssetUse as Mock).mockResolvedValue(terms)
    const onSubmit = vi.fn()
    renderModal({ ...saved, onSubmit })
    const user = userEvent.setup()

    await waitFor(() => expect(screen.getByLabelText('modals.assetDepreciation.grossPrice')).toHaveValue(15600))
    await user.click(screen.getByText('modals.editAccount.submit'))

    expect(onSubmit.mock.calls[0][7]).toEqual(terms)
  })

  it('adds a dated share change and keeps the share at purchase', async () => {
    const onSubmit = vi.fn()
    renderModal({ ...saved, onSubmit })
    const user = userEvent.setup()

    await user.click(await screen.findByText('modals.assetDepreciation.changeFromMonth'))
    await user.type(screen.getByLabelText('modals.assetDepreciation.changeMonth'), '2026-09')
    await user.type(screen.getByLabelText('modals.assetDepreciation.businessUse'), '80')
    await user.click(screen.getByText('modals.assetDepreciation.addChange'))
    await user.click(screen.getByText('modals.editAccount.submit'))

    expect(onSubmit.mock.calls[0][7]).toEqual({
      purchaseVat: null,
      businessUsePctBps: null,
      businessUseChanges: [{ fromMonth: '2026-09', businessUsePctBps: 8000 }],
    })
  })

  const konaTerms = {
    purchaseVat: { grossMinor: 1560000, vatRateBps: 2300, vatReclaimablePctBps: 5000 },
    businessUsePctBps: 8000,
    businessUseChanges: [],
  }

  it('refuses a blank VAT rate instead of dropping the VAT facts', async () => {
    ;(getAssetUse as Mock).mockResolvedValue(konaTerms)
    const onSubmit = vi.fn()
    renderModal({ ...saved, onSubmit })
    const user = userEvent.setup()

    await waitFor(() => expect(screen.getByLabelText('modals.assetDepreciation.vatRate')).toHaveValue(23))
    await user.clear(screen.getByLabelText('modals.assetDepreciation.vatRate'))
    await user.click(screen.getByText('modals.editAccount.submit'))

    expect(onSubmit).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalledWith('modals.assetDepreciation.vatIncomplete')
  })

  it('refuses a blank business use instead of reading it as 100 %', async () => {
    ;(getAssetUse as Mock).mockResolvedValue(konaTerms)
    const onSubmit = vi.fn()
    renderModal({ ...saved, onSubmit })
    const user = userEvent.setup()

    await waitFor(() => expect(screen.getByLabelText('modals.assetDepreciation.businessUse')).toHaveValue(80))
    await user.clear(screen.getByLabelText('modals.assetDepreciation.businessUse'))
    await user.click(screen.getByText('modals.editAccount.submit'))

    expect(onSubmit).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalledWith('modals.assetDepreciation.businessUseMissing')
  })

  const datedChange = { purchaseVat: null, businessUsePctBps: null, businessUseChanges: [{ fromMonth: '2027-01', businessUsePctBps: 6000 }] }

  it('keeps a dated change on screen when the period is removed, so it can be taken out', async () => {
    ;(getAssetUse as Mock).mockResolvedValue(datedChange)
    renderModal(saved)
    const user = userEvent.setup()

    await screen.findByText('modals.assetDepreciation.removeChange')
    await user.clear(screen.getByLabelText('modals.assetDepreciation.depreciationPeriodMonths'))

    expect(screen.getByText('modals.assetDepreciation.removeChange')).toBeInTheDocument()
  })

  it('takes a dated change only for a new month in YYYY-MM form', async () => {
    ;(getAssetUse as Mock).mockResolvedValue(datedChange)
    renderModal(saved)
    const user = userEvent.setup()

    await user.click(await screen.findByText('modals.assetDepreciation.changeFromMonth'))
    const month = screen.getByLabelText('modals.assetDepreciation.changeMonth')
    await user.type(screen.getByLabelText('modals.assetDepreciation.businessUse'), '80')
    const add = screen.getByText('modals.assetDepreciation.addChange')

    await user.type(month, 'abc')
    expect(add).toBeDisabled()
    await user.clear(month)
    await user.type(month, '2027-01')
    expect(add).toBeDisabled()
    await user.clear(month)
    await user.type(month, '2027-02')
    expect(add).toBeEnabled()
  })

  it('offers no dated change on an asset written off at once', async () => {
    renderModal({ ...saved, currentDepreciationPeriodMonths: 0 })

    await screen.findByLabelText('modals.assetDepreciation.privateUse')
    expect(screen.queryByText('modals.assetDepreciation.changeFromMonth')).not.toBeInTheDocument()
  })

  it('sends no terms for a save that cannot have read them', async () => {
    ;(getAssetUse as Mock).mockReturnValue(new Promise(() => {}))
    const onSubmit = vi.fn()
    renderModal({ ...saved, onSubmit })
    const user = userEvent.setup()

    await screen.findByLabelText('modals.assetDepreciation.purchasePrice')
    await user.click(screen.getByText('modals.editAccount.submit'))

    expect(onSubmit.mock.calls[0][7]).toBeNull()
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
