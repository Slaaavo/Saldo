import { useQuery } from '@tanstack/react-query'
import { findDifferenceCauses } from '@/shared/api'
import type { DifferenceCause } from '@/shared/types'
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue'

const TYPING_PAUSE_MS = 300

/** The causes of the gap between the typed bank balance and the balance the user sees, asked once typing pauses. `bankBalanceMinor` is NaN while nothing usable is typed. Undefined until the answer for the balance typed now arrives, so a list never sits under a newer difference. */
export const useDifferenceCauses = (accountId: number, bankBalanceMinor: number, seenBalanceMinor: number): DifferenceCause[] | undefined => {
  const typedMinor = useDebouncedValue(bankBalanceMinor, TYPING_PAUSE_MS)
  const { data } = useQuery({
    queryKey: ['bank-check-causes', accountId, typedMinor, seenBalanceMinor],
    queryFn: () => findDifferenceCauses(accountId, typedMinor, seenBalanceMinor),
    enabled: Number.isFinite(typedMinor) && typedMinor !== seenBalanceMinor,
    gcTime: 0,
  })
  return Object.is(typedMinor, bankBalanceMinor) ? data : undefined
}
