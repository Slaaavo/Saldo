import { useCallback, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import type { AsOf } from '../shared/types'
import { LIVE } from '../shared/utils/snapshot'
import { TimeMachineContext } from './useTimeMachine'

export const TimeMachineProvider = ({ children }: { children: ReactNode }) => {
  const [asOf, setAsOf] = useState<AsOf>(LIVE)
  const travelTo = useCallback((date: string) => setAsOf({ kind: 'at', until: date }), [])
  const backToLive = useCallback(() => setAsOf(LIVE), [])
  const value = useMemo(() => ({ asOf, travelTo, backToLive }), [asOf, travelTo, backToLive])
  return <TimeMachineContext.Provider value={value}>{children}</TimeMachineContext.Provider>
}
