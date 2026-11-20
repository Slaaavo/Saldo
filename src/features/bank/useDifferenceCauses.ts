import { skipToken, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query'
import { findDifferenceCauses } from '@/shared/api'
import type { DifferenceCause } from '@/shared/types'
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue'
import { hasErrorCode } from '@/shared/utils/errors'

const TYPING_PAUSE_MS = 300

const causesOrStaleBalance = async (queryClient: QueryClient, accountId: number, typedMinor: number, seenBalanceMinor: number): Promise<DifferenceCause[] | null> => {
  try {
    return await findDifferenceCauses(accountId, typedMinor, seenBalanceMinor)
  } catch (err) {
    if (!hasErrorCode(err, 'CHECK_CHANGED')) throw err
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    return null
  }
}

export const useDifferenceCauses = (accountId: number, bankBalanceMinor: number | null, seenBalanceMinor: number): DifferenceCause[] | undefined => {
  const queryClient = useQueryClient()
  const typedMinor = useDebouncedValue(bankBalanceMinor, TYPING_PAUSE_MS)
  const { data } = useQuery({
    queryKey: ['bank-check-causes', accountId, typedMinor, seenBalanceMinor],
    queryFn: typedMinor === null || typedMinor === seenBalanceMinor ? skipToken : () => causesOrStaleBalance(queryClient, accountId, typedMinor, seenBalanceMinor),
    gcTime: 0,
  })
  return typedMinor === bankBalanceMinor ? (data ?? undefined) : undefined
}
