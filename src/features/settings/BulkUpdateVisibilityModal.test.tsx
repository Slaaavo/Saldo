import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { SnapshotRow } from '../../shared/types'
import BulkUpdateVisibilityModal from './BulkUpdateVisibilityModal'

vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

const makeRow = (accountId: number, accountType: SnapshotRow['accountType'] = 'account') =>
  ({ accountId, accountName: `Account ${accountId}`, accountType, currencyCode: 'EUR' }) as SnapshotRow

const OTHER_PERSONS_ACCOUNT = 99

const renderModal = (currentExclusions: number[]) => {
  const onSave = vi.fn()
  render(<BulkUpdateVisibilityModal accounts={[makeRow(1), makeRow(2), makeRow(3, 'bucket')]} currentExclusions={currentExclusions} onSave={onSave} onClose={vi.fn()} />)
  return onSave
}

const selectAll = () => screen.getAllByRole('checkbox')[0]
const save = () => screen.getByRole('button', { name: 'modals.bulkUpdateVisibility.save' })

const savedIds = (onSave: ReturnType<typeof vi.fn>) => [...(onSave.mock.calls[0][0] as number[])].sort((a, b) => a - b)

describe('BulkUpdateVisibilityModal select all', () => {
  it('includes every visible account but keeps the exclusions of accounts that are not shown', async () => {
    const user = userEvent.setup()
    const onSave = renderModal([OTHER_PERSONS_ACCOUNT, 1])

    await user.click(selectAll())
    await user.click(save())

    expect(savedIds(onSave)).toEqual([OTHER_PERSONS_ACCOUNT])
  })

  it('excludes every visible account and keeps the exclusions of accounts that are not shown', async () => {
    const user = userEvent.setup()
    const onSave = renderModal([OTHER_PERSONS_ACCOUNT])

    await user.click(selectAll())
    await user.click(save())

    expect(savedIds(onSave)).toEqual([1, 2, 3, OTHER_PERSONS_ACCOUNT])
  })

  it('leaves no exclusions when none existed and everything is included again', async () => {
    const user = userEvent.setup()
    const onSave = renderModal([])

    await user.click(selectAll())
    await user.click(selectAll())
    await user.click(save())

    expect(savedIds(onSave)).toEqual([])
  })
})
