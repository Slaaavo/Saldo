import { useState } from 'react'
import type { ReactNode } from 'react'
import { PageHeaderContext } from './usePageHeader'

export const PageHeaderProvider = ({ children }: { children: ReactNode }) => {
  const [titleOverride, setTitleOverride] = useState<string | null>(null)
  return <PageHeaderContext.Provider value={{ titleOverride, setTitleOverride }}>{children}</PageHeaderContext.Provider>
}
