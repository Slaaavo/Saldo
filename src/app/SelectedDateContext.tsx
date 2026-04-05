import { useState } from 'react'
import type { ReactNode } from 'react'
import { todayIso } from '../shared/utils/format'
import { SelectedDateContext } from './useSelectedDate'

export const SelectedDateProvider = ({ children }: { children: ReactNode }) => {
  const [selectedDate, setSelectedDate] = useState(todayIso)
  return <SelectedDateContext.Provider value={{ selectedDate, setSelectedDate }}>{children}</SelectedDateContext.Provider>
}
