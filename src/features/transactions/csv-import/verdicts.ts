import type { ImportVerdict, VerdictKind } from '../../../shared/types'

const NEW_KINDS = { new: true, completesTransfer: true, inSaldo: false, transferLeg: false } satisfies Record<VerdictKind, boolean>

/** A new row is ticked by default and listed above the rows already in Saldo. */
export const isNewRow = (verdict: ImportVerdict): boolean => NEW_KINDS[verdict.kind]
