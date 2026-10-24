import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { EventWithData } from '../../shared/types'
import EditBalanceUpdateModal from './EditBalanceUpdateModal'

vi.mock('react-i18next', () => {
  const t = (key: string, opts?: Record<string, unknown>) => (opts ? `${key}:${JSON.stringify(opts)}` : key)
  return { useTranslation: () => ({ t }) }
})

const mockToastError = vi.fn()
vi.mock('sonner', () => ({
  toast: { error: (...args: unknown[]) => mockToastError(...args) },
}))

vi.mock('../buckets/useBucketLinks', () => ({
  useBucketLinks: () => ({
    loadingLinks: false,
    visibleLinks: [],
    availableToLink: [],
    handleSourceAccountSelect: vi.fn(),
    handleAddLink: vi.fn(),
    handleUnlink: vi.fn(),
    handleRemoveNew: vi.fn(),
    getLinkedAccountIds: () => [],
  }),
}))

vi.mock('../buckets/BucketAllocationEditor', () => ({
  default: () => null,
}))

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

const event = {
  id: 5,
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  eventType: 'balance_update',
  eventDate: '2026-01-15T10:30:00',
  amountMinor: 10000,
  note: null,
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
} as EventWithData

const submitButton = () => screen.getByRole('button', { name: 'modals.editBalanceUpdate.submit' })

describe('EditBalanceUpdateModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('submits the edited values', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn().mockResolvedValue(undefined)
    render(<EditBalanceUpdateModal event={event} accounts={[]} onSubmit={onSubmit} onClose={vi.fn()} />)

    await user.click(submitButton())

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(5, 10000, '2026-01-15T10:30:00', ''))
  })

  it('re-enables Save and toasts the failure when the save is rejected', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn().mockRejectedValue(new Error('db locked'))
    render(<EditBalanceUpdateModal event={event} accounts={[]} onSubmit={onSubmit} onClose={vi.fn()} />)

    await user.click(submitButton())

    await waitFor(() => expect(mockToastError).toHaveBeenCalledWith('errors.updateEvent:{"error":"db locked"}'))
    expect(submitButton()).toBeEnabled()
  })
})
