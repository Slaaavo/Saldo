import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import MinimumTaxEditor from './MinimumTaxEditor'
import type { MinimumTaxBandFormState } from './minimumTaxBands'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string, opts?: Record<string, unknown>) => (opts ? `${key} ${Object.values(opts).join(' ')}` : key) }),
}))

const band = (upperBoundMinor: string, amountMinor = '340'): MinimumTaxBandFormState => ({ upperBoundMinor, amountMinor })
const openBand = (): MinimumTaxBandFormState => ({ upperBoundMinor: '', amountMinor: '' })

describe('MinimumTaxEditor', () => {
  const onBandsChange = vi.fn()

  beforeEach(() => {
    onBandsChange.mockClear()
  })

  it('offers to add a band only while there are none', () => {
    const { rerender } = render(<MinimumTaxEditor bands={[]} onBandsChange={onBandsChange} minorUnits={2} />)
    expect(screen.getByText('taxModels.addMinimumTaxBand')).toBeInTheDocument()

    rerender(<MinimumTaxEditor bands={[band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    expect(screen.queryByText('taxModels.addMinimumTaxBand')).not.toBeInTheDocument()
  })

  it('adds an open band to an empty list', () => {
    render(<MinimumTaxEditor bands={[]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.click(screen.getByText('taxModels.addMinimumTaxBand'))

    expect(onBandsChange).toHaveBeenCalledWith([openBand()])
  })

  it('appends an open band when the last band gets a revenue limit', () => {
    render(<MinimumTaxEditor bands={[band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.change(screen.getByPlaceholderText('taxModels.bracketUpperBoundPlaceholder'), { target: { value: '50000' } })

    expect(onBandsChange).toHaveBeenCalledWith([band('50000'), openBand()])
  })

  it('keeps the later bands when an earlier revenue limit is cleared', () => {
    render(<MinimumTaxEditor bands={[band('1000'), band('2000'), band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.change(screen.getAllByDisplayValue('1000')[0], { target: { value: '' } })

    expect(onBandsChange).toHaveBeenCalledOnce()
    expect(onBandsChange.mock.calls[0][0]).toEqual([band(''), band('2000'), band('')])
  })

  it('changes the minimum tax of one band without adding another', () => {
    render(<MinimumTaxEditor bands={[band('1000', '340'), band('', '960')]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.change(screen.getByDisplayValue('960'), { target: { value: '980' } })

    expect(onBandsChange).toHaveBeenCalledWith([band('1000', '340'), band('', '980')])
  })

  it('steps and labels the amounts in the currency of the model', () => {
    render(<MinimumTaxEditor bands={[band('1000'), band('')]} onBandsChange={onBandsChange} minorUnits={0} currencyCode="JPY" />)

    expect(screen.getByDisplayValue('1000')).toHaveAttribute('step', '1')
    expect(screen.getAllByText('JPY').length).toBeGreaterThan(0)
  })

  it('names every input by its band number and column', () => {
    render(<MinimumTaxEditor bands={[band('1000', '340'), band('', '960')]} onBandsChange={onBandsChange} minorUnits={2} />)

    expect(screen.getByLabelText('taxModels.minimumTaxBandRow 1, taxModels.minimumTaxUpTo')).toHaveValue(1000)
    expect(screen.getByLabelText('taxModels.minimumTaxBandRow 1, taxModels.minimumTaxAmount')).toHaveValue(340)
    expect(screen.getByLabelText('taxModels.minimumTaxBandRow 2, taxModels.minimumTaxUpTo')).toHaveValue(null)
    expect(screen.getByLabelText('taxModels.minimumTaxBandRow 2, taxModels.minimumTaxAmount')).toHaveValue(960)
  })

  it('offers to remove every band, the first included', () => {
    render(<MinimumTaxEditor bands={[band('1000'), band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    expect(screen.getAllByText('taxModels.removeMinimumTaxBand')).toHaveLength(2)
  })

  it('removes an intermediate band and keeps the rest', () => {
    render(<MinimumTaxEditor bands={[band('1000'), band('2000'), band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.click(screen.getAllByText('taxModels.removeMinimumTaxBand')[0])

    expect(onBandsChange).toHaveBeenCalledWith([band('2000'), band('')])
  })

  it('promotes the previous band to catch-all when the last one is removed', () => {
    render(<MinimumTaxEditor bands={[band('1000'), band('2000'), band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.click(screen.getAllByText('taxModels.removeMinimumTaxBand')[2])

    expect(onBandsChange).toHaveBeenCalledWith([band('1000'), band('')])
  })

  it('leaves no bands when the only band is removed', () => {
    render(<MinimumTaxEditor bands={[band('')]} onBandsChange={onBandsChange} minorUnits={2} />)

    fireEvent.click(screen.getByText('taxModels.removeMinimumTaxBand'))

    expect(onBandsChange).toHaveBeenCalledWith([])
  })
})
