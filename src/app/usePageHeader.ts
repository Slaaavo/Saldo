import { createContext, useContext, useEffect } from 'react'

interface PageHeaderContextValue {
  titleOverride: string | null
  setTitleOverride: (title: string | null) => void
}

export const PageHeaderContext = createContext<PageHeaderContextValue | null>(null)

export const usePageHeader = () => {
  const ctx = useContext(PageHeaderContext)
  if (!ctx) throw new Error('usePageHeader must be used within a PageHeaderProvider')
  return ctx
}

export const usePageTitle = (title: string) => {
  const { setTitleOverride } = usePageHeader()
  useEffect(() => {
    setTitleOverride(title)
    return () => setTitleOverride(null)
  }, [title, setTitleOverride])
}
