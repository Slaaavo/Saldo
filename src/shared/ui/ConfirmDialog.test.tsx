import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import ConfirmDialog from './ConfirmDialog'

vi.mock('react-i18next', async () => (await import('../testing/i18nMock')).i18nMock)

describe('ConfirmDialog', () => {
  it('names the deletion in the title and on the button that runs it', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn()
    render(<ConfirmDialog action="delete" message="Delete the account?" onConfirm={onConfirm} onCancel={vi.fn()} />)

    expect(screen.getByRole('heading', { name: 'modals.confirm.deleteTitle' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'common.delete' }))

    expect(onConfirm).toHaveBeenCalledTimes(1)
  })

  it('keeps the plain Confirm wording for a question that deletes nothing', () => {
    render(<ConfirmDialog action="confirm" message="Fetch rates?" onConfirm={vi.fn()} onCancel={vi.fn()} />)

    expect(screen.getByRole('heading', { name: 'modals.confirm.title' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'modals.confirm.submit' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'common.delete' })).not.toBeInTheDocument()
  })
})
