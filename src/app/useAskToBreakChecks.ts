import { createContext, useContext } from 'react'
import type { AskToBreakChecks } from '../shared/api'

export const BreakingChecksContext = createContext<AskToBreakChecks | null>(null)

export const useAskToBreakChecks = (): AskToBreakChecks => {
  const ask = useContext(BreakingChecksContext)
  if (!ask) throw new Error('useAskToBreakChecks must be used within a BreakingChecksProvider')
  return ask
}
