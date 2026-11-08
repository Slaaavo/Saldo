import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Switch } from './switch'

describe('Switch', () => {
  it('asks for the opposite state on Space and on Enter', async () => {
    const user = userEvent.setup()
    const onCheckedChange = vi.fn()
    render(<Switch checked={false} onCheckedChange={onCheckedChange} aria-label="Count Holiday" />)
    const toggle = screen.getByRole('switch', { name: 'Count Holiday' })
    expect(toggle).toHaveAttribute('aria-checked', 'false')

    toggle.focus()
    await user.keyboard(' ')
    await user.keyboard('{Enter}')

    expect(onCheckedChange.mock.calls).toEqual([[true], [true]])
  })

  it('asks to switch off when clicked while on', async () => {
    const user = userEvent.setup()
    const onCheckedChange = vi.fn()
    render(<Switch checked={true} onCheckedChange={onCheckedChange} aria-label="Count Holiday" />)

    await user.click(screen.getByRole('switch', { name: 'Count Holiday' }))

    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true')
    expect(onCheckedChange).toHaveBeenCalledWith(false)
  })
})
