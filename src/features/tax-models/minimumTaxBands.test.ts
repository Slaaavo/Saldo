import { describe, it, expect } from 'vitest'
import { bandsFromForm, bandsToForm, emptyMinimumTaxBand } from './minimumTaxBands'

describe('bandsToForm', () => {
  it('shows each band up to the start of the next one and leaves the last band open', () => {
    const bands = [
      { lowerBoundMinor: 0, amountMinor: 34000 },
      { lowerBoundMinor: 5000000, amountMinor: 96000 },
      { lowerBoundMinor: 25000000, amountMinor: 192000 },
    ]

    expect(bandsToForm(bands, 2)).toEqual([
      { upperBoundMinor: '50000.00', amountMinor: '340.00' },
      { upperBoundMinor: '250000.00', amountMinor: '960.00' },
      { upperBoundMinor: '', amountMinor: '1920.00' },
    ])
  })

  it('has no rows for a model without minimum tax', () => {
    expect(bandsToForm([], 2)).toEqual([])
  })

  it('reads amounts in whole units for a currency without decimals', () => {
    expect(bandsToForm([{ lowerBoundMinor: 0, amountMinor: 340 }], 0)).toEqual([{ upperBoundMinor: '', amountMinor: '340' }])
  })
})

describe('bandsFromForm', () => {
  it('starts the first band at 0 and starts each later band where the one before it ends', () => {
    const result = bandsFromForm(
      [
        { upperBoundMinor: '50000', amountMinor: '340' },
        { upperBoundMinor: '250000.50', amountMinor: '960' },
        { upperBoundMinor: '', amountMinor: '1920' },
      ],
      2,
    )

    expect(result).toEqual({
      ok: true,
      bands: [
        { lowerBoundMinor: 0, amountMinor: 34000 },
        { lowerBoundMinor: 5000000, amountMinor: 96000 },
        { lowerBoundMinor: 25000050, amountMinor: 192000 },
      ],
    })
  })

  it('gives no bands for an empty list', () => {
    expect(bandsFromForm([], 2)).toEqual({ ok: true, bands: [] })
  })

  it('gives one open band for a single row', () => {
    expect(bandsFromForm([{ upperBoundMinor: '', amountMinor: '340' }], 2)).toEqual({ ok: true, bands: [{ lowerBoundMinor: 0, amountMinor: 34000 }] })
  })

  it('converts amounts in whole units for a currency without decimals', () => {
    const result = bandsFromForm(
      [
        { upperBoundMinor: '1000', amountMinor: '340' },
        { upperBoundMinor: '', amountMinor: '960' },
      ],
      0,
    )

    expect(result).toEqual({
      ok: true,
      bands: [
        { lowerBoundMinor: 0, amountMinor: 340 },
        { lowerBoundMinor: 1000, amountMinor: 960 },
      ],
    })
  })

  it('round-trips the bands of a model', () => {
    const bands = [
      { lowerBoundMinor: 0, amountMinor: 34000 },
      { lowerBoundMinor: 5000000, amountMinor: 96000 },
    ]

    expect(bandsFromForm(bandsToForm(bands, 2), 2)).toEqual({ ok: true, bands })
  })

  it.each(['', 'abc', '-1'])('rejects the minimum tax "%s" and names the band', (amountMinor) => {
    const result = bandsFromForm(
      [
        { upperBoundMinor: '1000', amountMinor: '340' },
        { upperBoundMinor: '', amountMinor },
      ],
      2,
    )

    expect(result).toEqual({ ok: false, error: { kind: 'amountInvalid', band: 2 } })
  })

  it('rejects a minimum tax above the limit', () => {
    const result = bandsFromForm([{ upperBoundMinor: '', amountMinor: '100000000000.01' }], 2)

    expect(result).toEqual({ ok: false, error: { kind: 'amountInvalid', band: 1 } })
  })

  it('rejects a revenue limit above the limit', () => {
    const result = bandsFromForm([{ upperBoundMinor: '100000000000.01', amountMinor: '340' }, emptyMinimumTaxBand()], 2)

    expect(result).toEqual({ ok: false, error: { kind: 'upperBoundInvalid', band: 1 } })
  })

  it('accepts a minimum tax of 0', () => {
    expect(bandsFromForm([{ upperBoundMinor: '', amountMinor: '0' }], 2)).toEqual({ ok: true, bands: [{ lowerBoundMinor: 0, amountMinor: 0 }] })
  })

  it('requires a revenue limit on every band but the last', () => {
    const result = bandsFromForm(
      [
        { upperBoundMinor: '', amountMinor: '340' },
        { upperBoundMinor: '', amountMinor: '960' },
      ],
      2,
    )

    expect(result).toEqual({ ok: false, error: { kind: 'upperBoundRequired', band: 1 } })
  })

  it('rejects a revenue limit that is not a number', () => {
    const result = bandsFromForm([{ upperBoundMinor: 'abc', amountMinor: '340' }, emptyMinimumTaxBand()], 2)

    expect(result).toEqual({ ok: false, error: { kind: 'upperBoundInvalid', band: 1 } })
  })

  it.each(['0', '-5'])('rejects a first revenue limit of %s, which leaves the band empty', (upperBoundMinor) => {
    const result = bandsFromForm(
      [
        { upperBoundMinor, amountMinor: '340' },
        { upperBoundMinor: '', amountMinor: '960' },
      ],
      2,
    )

    expect(result).toEqual({ ok: false, error: { kind: 'upperBoundNotIncreasing', band: 1 } })
  })

  it('rejects a revenue limit that does not rise above the one before it', () => {
    const result = bandsFromForm(
      [
        { upperBoundMinor: '1000', amountMinor: '340' },
        { upperBoundMinor: '1000', amountMinor: '960' },
        { upperBoundMinor: '', amountMinor: '1920' },
      ],
      2,
    )

    expect(result).toEqual({ ok: false, error: { kind: 'upperBoundNotIncreasing', band: 2 } })
  })

  it('rejects a revenue limit on the last band', () => {
    const result = bandsFromForm([{ upperBoundMinor: '1000', amountMinor: '340' }], 2)

    expect(result).toEqual({ ok: false, error: { kind: 'lastUpperBoundNotAllowed', band: 1 } })
  })
})
