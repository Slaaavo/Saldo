import { createContext, useContext, useState } from 'react'
import type { ReactNode } from 'react'
import { todayIso } from '../shared/utils/format'

interface SelectedDateContextValue {
  selectedDate: string
  setSelectedDate: (date: string) => void
}

const SelectedDateContext = createContext<SelectedDateContextValue | null>(null)

export function SelectedDateProvider({ children }: { children: ReactNode }) {
  const [selectedDate, setSelectedDate] = useState(todayIso)
  return <SelectedDateContext.Provider value={{ selectedDate, setSelectedDate }}>{children}</SelectedDateContext.Provider>
}

export function useSelectedDate(): SelectedDateContextValue {
  const ctx = useContext(SelectedDateContext)
  if (!ctx) throw new Error('useSelectedDate must be used within a SelectedDateProvider')
  return ctx
}
