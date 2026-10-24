import { describe, it, expect, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { SnapshotRow } from '../../shared/types'
import BulkUpdateBalanceModal from './BulkUpdateBalanceModal'

vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

vi.mock('sonner', () => ({
  toast: { error: vi.fn() },
}))

vi.mock('../../shared/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}))

const checking = {
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  balanceMinor: 0,
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
  isLinkedToAsset: false,
} as SnapshotRow

describe('BulkUpdateBalanceModal', () => {
  it('saves an untouched date with the time the date picker shows', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn().mockResolvedValue(undefined)
    render(<BulkUpdateBalanceModal accounts={[checking]} selectedDate="2026-01-15" exclusions={[]} onSubmit={onSubmit} onClose={vi.fn()} />)

    expect(screen.getByTestId('date-picker')).toHaveValue('2026-01-15T23:59:00')
    await user.type(screen.getByRole('spinbutton'), '42')
    await user.click(screen.getByRole('button', { name: 'modals.bulkUpdate.submit' }))

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith([{ accountId: 1, amountMinor: 4200 }], '2026-01-15T23:59:00', ''))
  })
})
