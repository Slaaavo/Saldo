import { createContext, useContext } from 'react'

export interface DemoContextValue {
  isDemoMode: boolean
  onEnterDemoMode: () => void
  onExitDemoMode: () => void
}

export const DemoContext = createContext<DemoContextValue | null>(null)

export const useDemo = (): DemoContextValue => {
  const ctx = useContext(DemoContext)
  if (!ctx) throw new Error('useDemo must be used within a DemoProvider')
  return ctx
}
