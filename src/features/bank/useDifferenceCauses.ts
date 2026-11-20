import { skipToken, useQuery } from '@tanstack/react-query'
import { findDifferenceCauses } from '@/shared/api'
import type { DifferenceCause } from '@/shared/types'
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue'

const TYPING_PAUSE_MS = 300

export const useDifferenceCauses = (accountId: number, bankBalanceMinor: number | null, seenBalanceMinor: number): DifferenceCause[] | undefined => {
  const typedMinor = useDebouncedValue(bankBalanceMinor, TYPING_PAUSE_MS)
  const { data } = useQuery({
    queryKey: ['bank-check-causes', accountId, typedMinor, seenBalanceMinor],
    queryFn: typedMinor === null || typedMinor === seenBalanceMinor ? skipToken : () => findDifferenceCauses(accountId, typedMinor, seenBalanceMinor),
    gcTime: 0,
  })
  return typedMinor === bankBalanceMinor ? data : undefined
}
