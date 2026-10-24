import { useState, useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQuery } from '@tanstack/react-query'
import { getDbLocation, pickDbFolder, changeDbLocation, resetDbLocation, checkDefaultDb } from '../../shared/api'
import type { ModalState } from '../../shared/types'
import { extractErrorMessage } from '../../shared/utils/errors'
import { reloadAfterDbSwap } from './reloadAfterDbSwap'

const useDbLocationQuery = () => {
  return useQuery({ queryKey: ['db-location'], queryFn: getDbLocation })
}

// Mounted once by the app shell, so the toast fires once per app start rather than on every Settings visit.
export const useDbFallbackWarning = () => {
  const { t } = useTranslation()
  const { data } = useDbLocationQuery()
  const warned = useRef(false)

  useEffect(() => {
    if (data?.fallbackWarning && !warned.current) {
      warned.current = true
      toast.warning(t('dataStorage.toasts.fallbackWarning'))
    }
  }, [data?.fallbackWarning, t])
}

interface UseDbLocationOptions {
  setModalState: (state: ModalState) => void
}

export const useDbLocation = ({ setModalState }: UseDbLocationOptions) => {
  const { t } = useTranslation()
  const { data: info } = useDbLocationQuery()
  const [actionLoading, setActionLoading] = useState(false)

  const handleChange = async () => {
    try {
      const result = await pickDbFolder()
      if (!result) return
      if (result.dbExists) {
        setModalState({ type: 'confirmSwitchDb', folder: result.folder })
      } else {
        setModalState({ type: 'dbLocationChoice', folder: result.folder, isReset: false })
      }
    } catch (err) {
      toast.error(t('dataStorage.errors.change', { error: extractErrorMessage(err) }))
    }
  }

  const handleReset = async () => {
    try {
      const dbExists = await checkDefaultDb()
      if (dbExists) {
        setModalState({ type: 'confirmResetDbLocation' })
      } else {
        setModalState({ type: 'dbLocationChoice', folder: '', isReset: true })
      }
    } catch (err) {
      toast.error(t('dataStorage.errors.reset', { error: extractErrorMessage(err) }))
    }
  }

  const handleConfirmSwitch = async (folder: string) => {
    setActionLoading(true)
    try {
      await changeDbLocation(folder, 'switch')
      await reloadAfterDbSwap()
    } catch (err) {
      toast.error(t('dataStorage.errors.change', { error: extractErrorMessage(err) }))
    } finally {
      setActionLoading(false)
    }
  }

  const handleLocationChoiceAction = async (action: string, folder: string, isReset: boolean) => {
    setActionLoading(true)
    try {
      if (isReset) {
        await resetDbLocation(action)
      } else {
        await changeDbLocation(folder, action)
      }
      await reloadAfterDbSwap()
    } catch (err) {
      toast.error(isReset ? t('dataStorage.errors.reset', { error: extractErrorMessage(err) }) : t('dataStorage.errors.change', { error: extractErrorMessage(err) }))
    } finally {
      setActionLoading(false)
    }
  }

  const handleConfirmReset = async () => {
    setActionLoading(true)
    try {
      await resetDbLocation('switch')
      await reloadAfterDbSwap()
    } catch (err) {
      toast.error(t('dataStorage.errors.reset', { error: extractErrorMessage(err) }))
    } finally {
      setActionLoading(false)
    }
  }

  return {
    path: info?.currentPath ?? '',
    isDefault: info?.isDefault ?? true,
    actionLoading,
    handleChange,
    handleReset,
    handleConfirmSwitch,
    handleLocationChoiceAction,
    handleConfirmReset,
  }
}
