import { createContext, useContext } from 'react'

interface SelectedDateContextValue {
  selectedDate: string
  setSelectedDate: (date: string) => void
}

export const SelectedDateContext = createContext<SelectedDateContextValue | null>(null)

export const useSelectedDate = () => {
  const ctx = useContext(SelectedDateContext)
  if (!ctx) throw new Error('useSelectedDate must be used within a SelectedDateProvider')
  return ctx
}
