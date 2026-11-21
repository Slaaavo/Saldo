import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { bulkCreateBalanceUpdates, ChangeDeclined, createBalanceUpdate, deleteEvent, deleteSplitGroup, updateEvent, updateTransfer, type AskToBreakChecks } from '.'
import type { BrokenCheck } from '../types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

const tatra: BrokenCheck = { accountId: 3, accountName: 'Tatra', day: '2026-11-14', differenceMinor: -1250, currencyCode: 'EUR', currencyMinorUnits: 2 }
const refusal = { code: 'BREAKS_VERIFIED_CHECKS', message: 'This change makes a verified bank check differ.', checks: [tatra] }
const editInput = { eventId: 5, amountMinor: 1000, eventDate: '2026-11-13T10:00:00', note: null }

const refuseUnlessAllowed = () =>
  (invoke as Mock).mockImplementation(async (_command: string, args: { allowBreakingChecks: boolean }) => {
    if (!args.allowBreakingChecks) throw refusal
  })

beforeEach(() => {
  vi.clearAllMocks()
})

describe('a write that would break a verified check', () => {
  it('asks with the broken checks and makes the change anyway when the user allows it', async () => {
    refuseUnlessAllowed()
    const ask = vi.fn<AskToBreakChecks>().mockResolvedValue(true)

    await expect(updateEvent(5, 1000, '2026-11-13T10:00:00', null, ask)).resolves.toBeUndefined()

    expect(ask).toHaveBeenCalledWith([tatra])
    expect((invoke as Mock).mock.calls).toEqual([
      ['update_event', { input: editInput, allowBreakingChecks: false }],
      ['update_event', { input: editInput, allowBreakingChecks: true }],
    ])
  })

  it('throws ChangeDeclined after one call when the user keeps the check', async () => {
    refuseUnlessAllowed()
    const ask = vi.fn<AskToBreakChecks>().mockResolvedValue(false)

    await expect(updateEvent(5, 1000, '2026-11-13T10:00:00', null, ask)).rejects.toBeInstanceOf(ChangeDeclined)

    expect(invoke).toHaveBeenCalledOnce()
  })

  it('passes any other error through without asking', async () => {
    const locked = { code: 'DATABASE', message: 'database is locked' }
    ;(invoke as Mock).mockRejectedValue(locked)
    const ask = vi.fn<AskToBreakChecks>()

    await expect(deleteEvent(5, ask)).rejects.toBe(locked)

    expect(ask).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledOnce()
  })
})

describe('guarded commands on the wire', () => {
  const ask = vi.fn<AskToBreakChecks>()
  const transfer = {
    fromEventId: 8,
    toEventId: 9,
    fromDate: '2026-11-13T10:00:00',
    toDate: '2026-11-13T10:00:00',
    amountMinor: -500,
    toAmountMinor: 500,
    note: null,
    originalCurrencyId: null,
    fxRateMantissa: null,
    fxRateExponent: null,
  }
  const entries = [{ accountId: 3, amountMinor: 100000 }]

  it.each([
    ['update_event', () => updateEvent(5, 1000, '2026-11-13T10:00:00', null, ask), { input: editInput }],
    ['delete_event', () => deleteEvent(5, ask), { eventId: 5 }],
    ['delete_split_group', () => deleteSplitGroup(9, ask), { splitGroupId: 9 }],
    ['update_transfer', () => updateTransfer(transfer, ask), { input: transfer }],
    [
      'create_balance_update',
      () => createBalanceUpdate(3, 100000, '2026-11-13T23:59:00', 'Payday', ask),
      { input: { accountId: 3, amountMinor: 100000, eventDate: '2026-11-13T23:59:00', note: 'Payday' } },
    ],
    [
      'bulk_create_balance_updates',
      () => bulkCreateBalanceUpdates(entries, '2026-11-13T23:59:00', null, ask),
      { input: { entries, eventDate: '2026-11-13T23:59:00', note: null } },
    ],
  ])('sends %s with allowBreakingChecks false next to its arguments', async (command, write, args) => {
    ;(invoke as Mock).mockResolvedValue(undefined)

    await write()

    expect(invoke).toHaveBeenCalledWith(command, { ...args, allowBreakingChecks: false })
  })
})
