import { useState } from 'react'
import type { ReactNode } from 'react'
import { SelectedPersonContext } from './useSelectedPerson'

export const SelectedPersonProvider = ({ children }: { children: ReactNode }) => {
  const [selectedPersonId, setSelectedPersonId] = useState<number | null>(null)
  return <SelectedPersonContext.Provider value={{ selectedPersonId, setSelectedPersonId }}>{children}</SelectedPersonContext.Provider>
}
