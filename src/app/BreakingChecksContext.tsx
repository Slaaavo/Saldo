import { useCallback, useRef, useState, type ReactNode } from 'react'
import type { AskToBreakChecks } from '../shared/api'
import type { BrokenCheck } from '../shared/types'
import BreakingChecksDialog from '../features/bank/BreakingChecksDialog'
import { BreakingChecksContext } from './useAskToBreakChecks'

export const BreakingChecksProvider = ({ children }: { children: ReactNode }) => {
  const [checks, setChecks] = useState<BrokenCheck[] | null>(null)
  const answerRef = useRef<((allow: boolean) => void) | null>(null)

  const ask = useCallback<AskToBreakChecks>((next) => {
    answerRef.current?.(false)
    return new Promise((resolve) => {
      answerRef.current = resolve
      setChecks(next)
    })
  }, [])

  const answer = (allow: boolean) => {
    answerRef.current?.(allow)
    answerRef.current = null
    setChecks(null)
  }

  return (
    <BreakingChecksContext.Provider value={ask}>
      {children}
      {checks && <BreakingChecksDialog checks={checks} onConfirm={() => answer(true)} onCancel={() => answer(false)} />}
    </BreakingChecksContext.Provider>
  )
}
