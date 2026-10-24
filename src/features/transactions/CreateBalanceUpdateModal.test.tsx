import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { SnapshotRow } from '../../shared/types'
import CreateBalanceUpdateModal from './CreateBalanceUpdateModal'

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

const makeRow = (overrides: Partial<SnapshotRow>): SnapshotRow =>
  ({ accountId: 1, accountName: 'Checking', accountType: 'account', currencyCode: 'EUR', currencyMinorUnits: 2, ...overrides }) as SnapshotRow

const checking = makeRow({ accountId: 1 })
const bucket = makeRow({ accountId: 2, accountName: 'Vacation', accountType: 'bucket' })

const renderModal = (props: Partial<React.ComponentProps<typeof CreateBalanceUpdateModal>> = {}) => {
  const onSubmit = vi.fn().mockResolvedValue(undefined)
  const onBucketSubmit = vi.fn().mockResolvedValue(undefined)
  const onClose = vi.fn()
  render(<CreateBalanceUpdateModal accounts={[checking, bucket]} preselectedAccountId={1} onSubmit={onSubmit} onBucketSubmit={onBucketSubmit} onClose={onClose} {...props} />)
  return { onSubmit, onBucketSubmit, onClose }
}

const submitButton = () => screen.getByRole('button', { name: 'modals.createBalanceUpdate.submit' })

describe('CreateBalanceUpdateModal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('re-enables Save and toasts the failure when the save is rejected', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn().mockRejectedValue(new Error('db locked'))
    renderModal({ onSubmit })

    await user.type(screen.getByLabelText('modals.createBalanceUpdate.amount'), '12.50')
    await user.click(submitButton())

    await waitFor(() => expect(mockToastError).toHaveBeenCalledWith('errors.createBalanceUpdate:{"error":"db locked"}'))
    expect(submitButton()).toBeEnabled()
  })

  it('keeps Save disabled while the save is in flight and passes the entered values', async () => {
    const user = userEvent.setup()
    let finish: () => void = () => {}
    const onSubmit = vi.fn().mockReturnValue(
      new Promise<void>((resolve) => {
        finish = resolve
      }),
    )
    renderModal({ onSubmit })

    await user.type(screen.getByLabelText('modals.createBalanceUpdate.amount'), '12.50')
    await user.click(submitButton())

    await waitFor(() => expect(submitButton()).toBeDisabled())
    expect(onSubmit).toHaveBeenCalledWith(1, 1250, expect.any(String), '')

    finish()
    await waitFor(() => expect(submitButton()).toBeEnabled())
  })

  it('leaves closing to the handler after a bucket balance update is saved', async () => {
    const user = userEvent.setup()
    const { onBucketSubmit, onClose } = renderModal({ preselectedAccountId: 2 })

    await user.click(submitButton())

    await waitFor(() => expect(onBucketSubmit).toHaveBeenCalledOnce())
    expect(onClose).not.toHaveBeenCalled()
  })

  it('keeps a bucket link conflict out of the toasts and re-enables Save', async () => {
    const user = userEvent.setup()
    const onBucketSubmit = vi
      .fn()
      .mockRejectedValue({ code: 'LINK_CONFLICT', message: JSON.stringify({ sourceAccountName: 'A', otherBucketName: 'B', conflictDate: '2026-01-01' }) })
    renderModal({ preselectedAccountId: 2, onBucketSubmit })

    await user.click(submitButton())

    await waitFor(() => expect(submitButton()).toBeEnabled())
    expect(mockToastError).not.toHaveBeenCalled()
  })
})
