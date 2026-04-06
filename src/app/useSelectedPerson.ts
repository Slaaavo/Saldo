import { createContext, useContext } from 'react'

interface SelectedPersonContextValue {
  selectedPersonId: number | null
  setSelectedPersonId: (id: number | null) => void
}

export const SelectedPersonContext = createContext<SelectedPersonContextValue | null>(null)

export const useSelectedPerson = () => {
  const ctx = useContext(SelectedPersonContext)
  if (!ctx) throw new Error('useSelectedPerson must be used within a SelectedPersonProvider')
  return ctx
}
