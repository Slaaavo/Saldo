import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { getAccountsSnapshot } from '../api'
import type { AsOf } from '../types'
import { useSelectedPerson } from '../../app/useSelectedPerson'

const LIVE_RATE_AGE_REFRESH_MS = 60 * 60 * 1000

export const useSnapshotQuery = (asOf: AsOf) => {
  const { selectedPersonId } = useSelectedPerson()
  return useQuery({
    queryKey: ['snapshot', asOf, selectedPersonId],
    queryFn: () => getAccountsSnapshot(asOf, selectedPersonId ?? undefined),
    placeholderData: keepPreviousData,
    refetchInterval: asOf.kind === 'live' ? LIVE_RATE_AGE_REFRESH_MS : false,
  })
}
