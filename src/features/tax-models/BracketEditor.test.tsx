import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import BracketEditor, { type BracketFormState } from './BracketEditor'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const bracket = (upperBoundMinor: string): BracketFormState => ({ upperBoundMinor, rateType: 'flat', flatRateBps: '10', tiers: [] })

describe('BracketEditor', () => {
  const onBracketsChange = vi.fn()

  beforeEach(() => {
    onBracketsChange.mockClear()
  })

  it('appends an empty bracket when the last bracket gets an upper bound', () => {
    render(<BracketEditor brackets={[bracket('')]} onBracketsChange={onBracketsChange} />)

    fireEvent.change(screen.getByPlaceholderText('taxModels.bracketUpperBoundPlaceholder'), { target: { value: '1000' } })

    expect(onBracketsChange).toHaveBeenCalledWith([bracket('1000'), { upperBoundMinor: '', rateType: 'flat', flatRateBps: '', tiers: [] }])
  })

  it('keeps later brackets when an intermediate upper bound is cleared', () => {
    render(<BracketEditor brackets={[bracket('1000'), bracket('2000'), bracket('')]} onBracketsChange={onBracketsChange} />)

    const firstBracketUpperBound = screen.getAllByDisplayValue('1000')[0]
    fireEvent.change(firstBracketUpperBound, { target: { value: '' } })

    expect(onBracketsChange).toHaveBeenCalledOnce()
    expect(onBracketsChange.mock.calls[0][0]).toEqual([bracket(''), bracket('2000'), bracket('')])
  })

  it('does not offer to remove the first bracket', () => {
    render(<BracketEditor brackets={[bracket('1000'), bracket('')]} onBracketsChange={onBracketsChange} />)

    expect(screen.getAllByText('taxModels.removeBracket')).toHaveLength(1)
  })

  it('removes an intermediate bracket and keeps the rest', () => {
    render(<BracketEditor brackets={[bracket('1000'), bracket('2000'), bracket('')]} onBracketsChange={onBracketsChange} />)

    fireEvent.click(screen.getAllByText('taxModels.removeBracket')[0])

    expect(onBracketsChange).toHaveBeenCalledWith([bracket('1000'), bracket('')])
  })

  it('promotes the previous bracket to catch-all when the last one is removed', () => {
    render(<BracketEditor brackets={[bracket('1000'), bracket('2000'), bracket('')]} onBracketsChange={onBracketsChange} />)

    fireEvent.click(screen.getAllByText('taxModels.removeBracket')[1])

    expect(onBracketsChange).toHaveBeenCalledWith([bracket('1000'), bracket('')])
  })
})
