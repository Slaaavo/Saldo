import { MAX_AMOUNT_MINOR, type MinimumTaxBand } from '@/shared/types'
import { fromMinorUnits, toMinorUnits } from '@/shared/utils/format'

export interface MinimumTaxBandFormState {
  upperBoundMinor: string
  amountMinor: string
}

export interface MinimumTaxBandError {
  kind: 'amountInvalid' | 'upperBoundRequired' | 'upperBoundInvalid' | 'upperBoundNotIncreasing' | 'lastUpperBoundNotAllowed'
  band: number
}

export type MinimumTaxBandsFromForm = { ok: true; bands: MinimumTaxBand[] } | { ok: false; error: MinimumTaxBandError }

export const emptyMinimumTaxBand = (): MinimumTaxBandFormState => ({ upperBoundMinor: '', amountMinor: '' })

export const bandsToForm = (bands: MinimumTaxBand[], minorUnits: number): MinimumTaxBandFormState[] =>
  bands.map((band, i) => ({
    upperBoundMinor: i < bands.length - 1 ? fromMinorUnits(bands[i + 1].lowerBoundMinor, minorUnits) : '',
    amountMinor: fromMinorUnits(band.amountMinor, minorUnits),
  }))

export const bandsFromForm = (forms: MinimumTaxBandFormState[], minorUnits: number): MinimumTaxBandsFromForm => {
  const bands: MinimumTaxBand[] = []
  let lowerBoundMinor = 0

  for (const [i, form] of forms.entries()) {
    const band = i + 1
    const isLast = i === forms.length - 1

    const amountMinor = toMinorUnits(form.amountMinor, minorUnits)
    if (!Number.isFinite(amountMinor) || amountMinor < 0 || amountMinor > MAX_AMOUNT_MINOR) return { ok: false, error: { kind: 'amountInvalid', band } }
    bands.push({ lowerBoundMinor, amountMinor })

    if (isLast) {
      if (form.upperBoundMinor !== '') return { ok: false, error: { kind: 'lastUpperBoundNotAllowed', band } }
      continue
    }
    if (form.upperBoundMinor === '') return { ok: false, error: { kind: 'upperBoundRequired', band } }
    const upperBoundMinor = toMinorUnits(form.upperBoundMinor, minorUnits)
    if (!Number.isFinite(upperBoundMinor) || upperBoundMinor > MAX_AMOUNT_MINOR) return { ok: false, error: { kind: 'upperBoundInvalid', band } }
    if (upperBoundMinor <= lowerBoundMinor) return { ok: false, error: { kind: 'upperBoundNotIncreasing', band } }
    lowerBoundMinor = upperBoundMinor
  }

  return { ok: true, bands }
}
