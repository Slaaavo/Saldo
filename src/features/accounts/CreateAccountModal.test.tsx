import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import type { PersonRow } from '../../shared/types'
import { getConsolidationCurrency, listCurrencies, listPersons } from '../../shared/api'
import { SelectedPersonContext } from '../../app/useSelectedPerson'
import CreateAccountModal from './CreateAccountModal'

vi.mock('../../shared/api', () => ({
  listCurrencies: vi.fn(),
  getConsolidationCurrency: vi.fn(),
  listPersons: vi.fn(),
}))

vi.mock('react-i18next', async () => (await import('../../shared/testing/i18nMock')).i18nMock)

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const EUR = { id: 1, code: 'EUR', name: 'Euro', minorUnits: 2, isCustom: false }

const person = (id: number, name: string, isDefault = false): PersonRow => ({
  id,
  name,
  personType: isDefault ? 'physical' : 'legal',
  isDefault,
  createdAt: '2026-01-01T00:00:00',
  defaultRevenueAccountId: 0,
  defaultExpenseAccountId: 0,
  vatPayer: !isDefault,
})

const PERSONAL = person(1, 'Personal', true)
const COMPANY = person(2, 'Sumbal consulting')

const renderModal = (options?: { accountType?: 'account' | 'bucket'; persons?: PersonRow[]; selectedPersonId?: number | null }) => {
  ;(listPersons as Mock).mockResolvedValue(options?.persons ?? [PERSONAL, COMPANY])
  const onSubmit = vi.fn()
  const onSubmitIncomeBudget = vi.fn()
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 0 } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <SelectedPersonContext.Provider value={{ selectedPersonId: options?.selectedPersonId ?? null, setSelectedPersonId: vi.fn() }}>{children}</SelectedPersonContext.Provider>
    </QueryClientProvider>
  )
  render(<CreateAccountModal accountType={options?.accountType ?? 'bucket'} onSubmit={onSubmit} onSubmitIncomeBudget={onSubmitIncomeBudget} onClose={vi.fn()} />, { wrapper })
  return { onSubmit, onSubmitIncomeBudget }
}

const kindButton = (kind: 'balance' | 'incomeBudget') => screen.getByRole('button', { name: `modals.createBucket.kinds.${kind}` })

const fillName = async (user: ReturnType<typeof userEvent.setup>, name: string) => {
  await screen.findByDisplayValue('EUR — Euro')
  await user.type(screen.getByLabelText('modals.createBucket.nameLabel'), name)
}

describe('CreateAccountModal kind', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(listCurrencies as Mock).mockResolvedValue([EUR])
    ;(getConsolidationCurrency as Mock).mockResolvedValue(EUR)
    Element.prototype.hasPointerCapture = () => false
    Element.prototype.releasePointerCapture = () => {}
    Element.prototype.scrollIntoView = () => {}
  })

  afterEach(() => {
    for (const name of ['hasPointerCapture', 'releasePointerCapture', 'scrollIntoView']) Reflect.deleteProperty(Element.prototype, name)
  })

  it('offers a bucket the kind Normal or Income budget, starting on Normal', async () => {
    renderModal()

    expect(await screen.findByRole('group', { name: 'modals.createBucket.kind' })).toBeInTheDocument()
    expect(kindButton('balance')).toHaveAttribute('aria-pressed', 'true')
    expect(kindButton('incomeBudget')).toHaveAttribute('aria-pressed', 'false')
  })

  it('offers no kind when creating an account', async () => {
    renderModal({ accountType: 'account' })

    await screen.findByDisplayValue('EUR — Euro')
    expect(screen.queryByRole('group', { name: 'modals.createBucket.kind' })).not.toBeInTheDocument()
  })

  it('hides the starting balance of an income budget and always asks for its person', async () => {
    const user = userEvent.setup()
    renderModal({ persons: [PERSONAL] })
    await screen.findByDisplayValue('EUR — Euro')
    expect(screen.getByLabelText('modals.createAccount.initialBalance')).toBeInTheDocument()
    expect(screen.queryByRole('combobox', { name: 'persons.selector' })).not.toBeInTheDocument()

    await user.click(kindButton('incomeBudget'))

    expect(kindButton('incomeBudget')).toHaveAttribute('aria-pressed', 'true')
    expect(screen.queryByLabelText('modals.createAccount.initialBalance')).not.toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: 'persons.selector' })).toHaveTextContent('modals.createBucket.pickPerson')
  })

  it('does not create an income budget without a person and says why', async () => {
    const user = userEvent.setup()
    const { onSubmit, onSubmitIncomeBudget } = renderModal()
    await fillName(user, 'Consulting income')

    await user.click(kindButton('incomeBudget'))
    await user.click(screen.getByRole('button', { name: 'modals.createAccount.submit' }))

    expect(screen.getByRole('alert')).toHaveTextContent('modals.createBucket.personRequired')
    expect(screen.getByRole('combobox', { name: 'persons.selector' })).toHaveAttribute('aria-invalid', 'true')
    expect(onSubmitIncomeBudget).not.toHaveBeenCalled()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('creates an income budget for the picked person with no starting balance', async () => {
    const user = userEvent.setup()
    const { onSubmit, onSubmitIncomeBudget } = renderModal()
    await fillName(user, 'Consulting income')
    await user.click(kindButton('incomeBudget'))
    await user.click(screen.getByRole('button', { name: 'modals.createAccount.submit' }))

    await user.click(screen.getByRole('combobox', { name: 'persons.selector' }))
    await user.click(await screen.findByRole('option', { name: 'Sumbal consulting' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'modals.createAccount.submit' }))

    expect(onSubmitIncomeBudget).toHaveBeenCalledWith('Consulting income', 1, 2)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("starts an income budget on the dashboard's person", async () => {
    const user = userEvent.setup()
    const { onSubmitIncomeBudget } = renderModal({ selectedPersonId: 2 })
    await fillName(user, 'Consulting income')

    await user.click(kindButton('incomeBudget'))
    await user.click(screen.getByRole('button', { name: 'modals.createAccount.submit' }))

    expect(onSubmitIncomeBudget).toHaveBeenCalledWith('Consulting income', 1, 2)
  })

  it('still creates a normal bucket with its starting balance for the default person', async () => {
    const user = userEvent.setup()
    const { onSubmit, onSubmitIncomeBudget } = renderModal()
    await fillName(user, 'Vacation')

    await user.type(screen.getByLabelText('modals.createAccount.initialBalance'), '100')
    await user.click(screen.getByRole('button', { name: 'modals.createAccount.submit' }))

    expect(onSubmit).toHaveBeenCalledWith('Vacation', 1, 10000, 'bucket', undefined, undefined, 1)
    expect(onSubmitIncomeBudget).not.toHaveBeenCalled()
  })
})
