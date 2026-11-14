import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import StandaloneEventCard from './StandaloneEventCard'
import type { EventWithData } from '../../types'

vi.mock('react-i18next', async () => (await import('../../testing/i18nMock')).i18nMock)

vi.mock('../NumberValue', () => ({
  default: ({ value }: { value: number }) => <span data-testid="number-value">{value}</span>,
}))

const unexplained: EventWithData = {
  id: 12,
  accountId: 1,
  accountName: 'Checking',
  accountType: 'account',
  eventType: 'unexplained_difference',
  eventDate: '2026-11-14T10:30:00',
  amountMinor: -1250,
  note: null,
  createdAt: '2026-11-14T09:30:00.000',
  currencyCode: 'EUR',
  currencyMinorUnits: 2,
  counterpartAccountId: null,
  counterpartAccountName: null,
  bucketId: null,
  bucketName: null,
  originalCurrencyId: null,
  originalCurrencyCode: null,
  originalAmountMinor: null,
  originalCurrencyMinorUnits: null,
  fxRateMantissa: null,
  fxRateExponent: null,
  linkedEventId: null,
  splitGroupId: null,
  splitGroupNote: null,
  vatRateBps: null,
  vatReclaimablePctBps: null,
  expenseDeductiblePctBps: null,
  prepaidUntil: null,
  isLinkedToTaxable: false,
  linkedTaxableEventId: null,
  hasLinkedCashflows: false,
  linkedCashflowCount: 0,
  linkedAssetId: null,
  linkedPrepaidEventId: null,
  isSystemGenerated: false,
  reclaimedVat: null,
  privateUse: false,
}

describe('StandaloneEventCard', () => {
  it('labels an unexplained difference and offers delete but no edit', async () => {
    const user = userEvent.setup()
    const onEditEvent = vi.fn()
    const onDeleteEvent = vi.fn()
    render(<StandaloneEventCard ev={unexplained} onEditEvent={onEditEvent} onDeleteEvent={onDeleteEvent} onDeleteTransferEvent={vi.fn()} />)

    expect(screen.getByText('events.type.unexplained_difference')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.edit' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'common.delete' }))

    expect(onDeleteEvent).toHaveBeenCalledWith(12, 'unexplained_difference')
    expect(onEditEvent).not.toHaveBeenCalled()
  })
})
