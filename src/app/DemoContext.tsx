import { createContext, useContext } from 'react'
import type { ReactNode } from 'react'

interface DemoContextValue {
  isDemoMode: boolean
  onEnterDemoMode: () => void
  onExitDemoMode: () => void
}

const DemoContext = createContext<DemoContextValue | null>(null)

interface DemoProviderProps {
  isDemoMode: boolean
  onEnterDemoMode: () => void
  onExitDemoMode: () => void
  children: ReactNode
}

export function DemoProvider({ isDemoMode, onEnterDemoMode, onExitDemoMode, children }: DemoProviderProps) {
  return <DemoContext.Provider value={{ isDemoMode, onEnterDemoMode, onExitDemoMode }}>{children}</DemoContext.Provider>
}

export function useDemo(): DemoContextValue {
  const ctx = useContext(DemoContext)
  if (!ctx) throw new Error('useDemo must be used within a DemoProvider')
  return ctx
}
