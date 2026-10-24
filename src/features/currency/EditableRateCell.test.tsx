import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { EditableRateCell } from './EditableRateCell'

// The cell stays in editing mode after a key press on purpose: WebView2 (Chromium) fires blur
// while the focused input is being removed, which jsdom and WebKit do not, so the tests fire it by hand.
describe('EditableRateCell', () => {
  const onSave = vi.fn()
  const onCancel = vi.fn()

  const renderCell = () =>
    render(
      <table>
        <tbody>
          <tr>
            <EditableRateCell
              date="2026-01-02"
              code="USD"
              row={undefined}
              isEditing
              editValue="1.15"
              onCellClick={vi.fn()}
              onEditValueChange={vi.fn()}
              onSave={onSave}
              onCancel={onCancel}
              manualLabel="manual"
            />
          </tr>
        </tbody>
      </table>,
    )

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('saves once when Enter is followed by the blur of the unmounting input', () => {
    renderCell()
    const input = screen.getByDisplayValue('1.15')

    fireEvent.keyDown(input, { key: 'Enter' })
    fireEvent.blur(input)

    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith('2026-01-02', 'USD', '1.15')
  })

  it('does not save when Escape is followed by the blur of the unmounting input', () => {
    renderCell()
    const input = screen.getByDisplayValue('1.15')

    fireEvent.keyDown(input, { key: 'Escape' })
    fireEvent.blur(input)

    expect(onCancel).toHaveBeenCalledTimes(1)
    expect(onSave).not.toHaveBeenCalled()
  })

  it('still saves on a plain blur', () => {
    renderCell()

    fireEvent.blur(screen.getByDisplayValue('1.15'))

    expect(onSave).toHaveBeenCalledTimes(1)
    expect(onSave).toHaveBeenCalledWith('2026-01-02', 'USD', '1.15')
  })
})
