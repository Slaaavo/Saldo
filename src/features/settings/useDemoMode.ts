import { useState, useEffect } from 'react'
import { toast } from 'sonner'
import { enterDemoMode, exitDemoMode, isDemoMode as checkIsDemoMode } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import { reloadAfterDbSwap } from './reloadAfterDbSwap'

// The backend keeps the demo flag across the reload that follows each swap.
export const useDemoMode = () => {
  const [isDemoMode, setIsDemoMode] = useState(false)

  useEffect(() => {
    checkIsDemoMode().then(setIsDemoMode).catch(console.error)
  }, [])

  const handleEnter = async () => {
    try {
      await enterDemoMode()
    } catch (err) {
      toast.error(extractErrorMessage(err))
      return
    }
    window.location.reload()
  }

  const handleExit = async () => {
    try {
      await exitDemoMode()
    } catch (err) {
      toast.error(extractErrorMessage(err))
      return
    }
    await reloadAfterDbSwap()
  }

  return { isDemoMode, handleEnter, handleExit }
}
