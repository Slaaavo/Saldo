import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DatePicker } from './date-picker'

vi.mock('react-i18next', () => {
  const t = (key: string) => key
  return { useTranslation: () => ({ t }) }
})

const openPicker = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(screen.getByRole('button', { name: /2026/ }))
}

describe('DatePicker with time', () => {
  it('shows the time carried by the value', async () => {
    const user = userEvent.setup()
    render(<DatePicker value="2026-01-15T10:30:00" onChange={vi.fn()} withTime />)

    await openPicker(user)

    const [hour, minute] = screen.getAllByRole('combobox')
    expect(hour).toHaveTextContent('10')
    expect(minute).toHaveTextContent('30')
  })

  it('follows the value when it changes to a different time', async () => {
    const user = userEvent.setup()
    const { rerender } = render(<DatePicker value="2026-01-15T10:30:00" onChange={vi.fn()} withTime />)
    await openPicker(user)

    rerender(<DatePicker value="2026-01-16T14:05:00" onChange={vi.fn()} withTime />)

    const [hour, minute] = screen.getAllByRole('combobox')
    expect(hour).toHaveTextContent('14')
    expect(minute).toHaveTextContent('05')
  })

  it('shows the default time for a date-only value', async () => {
    const user = userEvent.setup()
    render(<DatePicker value="2026-01-15" onChange={vi.fn()} withTime />)

    await openPicker(user)

    const [hour, minute] = screen.getAllByRole('combobox')
    expect(hour).toHaveTextContent('23')
    expect(minute).toHaveTextContent('59')
  })
})
