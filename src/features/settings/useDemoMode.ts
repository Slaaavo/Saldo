import { useState, useEffect } from 'react'
import { toast } from 'sonner'
import { enterDemoMode, exitDemoMode, getAppSetting, isDemoMode as checkIsDemoMode } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'

// localStorage holds the demo session's theme after leaving demo mode; refresh it so the reloaded page does not flash it.
// A failure here must not stop the reload, because the database swap has already happened.
const cacheRealTheme = async () => {
  try {
    localStorage.setItem('theme', (await getAppSetting('theme')) ?? 'system')
  } catch (err) {
    console.error('Failed to restore theme after demo mode:', err)
  }
}

// The backend swaps the whole database, so every cached query and every setting held in component state belongs to the other
// database afterwards. Reloading the window is the only way to drop all of it; the backend keeps the demo flag across a reload.
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
    await cacheRealTheme()
    window.location.reload()
  }

  return { isDemoMode, handleEnter, handleExit }
}
