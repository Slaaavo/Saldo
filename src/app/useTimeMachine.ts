import { createContext, useContext } from 'react'
import type { AsOf } from '../shared/types'

interface TimeMachineContextValue {
  asOf: AsOf
  travelTo: (date: string) => void
  backToLive: () => void
}

export const TimeMachineContext = createContext<TimeMachineContextValue | null>(null)

export const useTimeMachine = () => {
  const ctx = useContext(TimeMachineContext)
  if (!ctx) throw new Error('useTimeMachine must be used within a TimeMachineProvider')
  return ctx
}
