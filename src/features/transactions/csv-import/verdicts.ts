import type { ImportVerdict, VerdictKind } from '../../../shared/types'

const NEW_KINDS = { new: true, completesTransfer: true, inSaldo: false, transferLeg: false } satisfies Record<VerdictKind, boolean>

export const isNewRow = (verdict: ImportVerdict): boolean => NEW_KINDS[verdict.kind]
