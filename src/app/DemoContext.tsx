import type { ReactNode } from 'react'
import { DemoContext } from './useDemo'

interface DemoProviderProps {
  isDemoMode: boolean
  onEnterDemoMode: () => void
  onExitDemoMode: () => void
  children: ReactNode
}

export const DemoProvider = ({ isDemoMode, onEnterDemoMode, onExitDemoMode, children }: DemoProviderProps) => {
  return <DemoContext.Provider value={{ isDemoMode, onEnterDemoMode, onExitDemoMode }}>{children}</DemoContext.Provider>
}
