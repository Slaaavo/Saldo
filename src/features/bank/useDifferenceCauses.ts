import { useQuery } from '@tanstack/react-query'
import { findDifferenceCauses } from '@/shared/api'
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue'

const TYPING_PAUSE_MS = 300

/** The causes of the gap between the typed bank balance and the balance the user sees, asked once typing pauses. `bankBalanceMinor` is NaN while nothing usable is typed. */
export const useDifferenceCauses = (accountId: number, bankBalanceMinor: number, seenBalanceMinor: number) => {
  const typedMinor = useDebouncedValue(bankBalanceMinor, TYPING_PAUSE_MS)
  return useQuery({
    queryKey: ['bank-check-causes', accountId, typedMinor, seenBalanceMinor],
    queryFn: () => findDifferenceCauses(accountId, typedMinor, seenBalanceMinor),
    enabled: Number.isFinite(typedMinor) && typedMinor !== seenBalanceMinor,
  })
}
