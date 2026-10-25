import { describe, it, expect } from 'vitest'
import { groupRowsByKind, valueOf } from './snapshot'
import { NO_RATE, converted, makeRow, sameCurrency } from '../testing/snapshotFixtures'

describe('valueOf', () => {
  it('gives the value of a same-currency amount', () => {
    expect(valueOf(sameCurrency(1500))).toBe(1500)
  })

  it('gives the converted value of an amount with a rate', () => {
    expect(valueOf(converted(1380))).toBe(1380)
  })

  it('gives nothing for an amount without a rate, so it cannot be mistaken for zero', () => {
    expect(valueOf(NO_RATE)).toBeNull()
  })
})

describe('groupRowsByKind', () => {
  it('splits rows into accounts, buckets and assets and keeps their order', () => {
    const rows = [
      makeRow({ accountId: 1, accountType: 'account' }),
      makeRow({ accountId: 2, accountType: 'bucket' }),
      makeRow({ accountId: 3, accountType: 'asset' }),
      makeRow({ accountId: 4, accountType: 'account' }),
    ]

    const { accounts, buckets, assets } = groupRowsByKind(rows)

    expect(accounts.map((r) => r.accountId)).toEqual([1, 4])
    expect(buckets.map((r) => r.accountId)).toEqual([2])
    expect(assets.map((r) => r.accountId)).toEqual([3])
  })
})
