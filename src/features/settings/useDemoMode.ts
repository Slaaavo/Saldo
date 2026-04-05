import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { enterDemoMode, exitDemoMode, isDemoMode as checkIsDemoMode } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'

interface UseDemoModeOptions {
  loadDbLocation: () => Promise<void>
  onEntered: () => void
}

export const useDemoMode = ({ loadDbLocation, onEntered }: UseDemoModeOptions) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [isDemoMode, setIsDemoMode] = useState(false)

  useEffect(() => {
    checkIsDemoMode().then(setIsDemoMode).catch(console.error)
  }, [])

  const handleEnter = async () => {
    try {
      await enterDemoMode()
      setIsDemoMode(true)
      await loadDbLocation()
      onEntered()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await queryClient.invalidateQueries({ queryKey: ['consolidation-currency'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  const handleExit = async () => {
    try {
      await exitDemoMode()
      setIsDemoMode(false)
      await loadDbLocation()
      toast.success(t('demo.exitedToast'))
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      await queryClient.invalidateQueries({ queryKey: ['consolidation-currency'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  return { isDemoMode, handleEnter, handleExit }
}
