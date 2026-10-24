import { useState, useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getDbLocation, pickDbFolder, changeDbLocation, resetDbLocation, checkDefaultDb } from '../../shared/api'
import type { ModalState } from '../../shared/types'
import { extractErrorMessage } from '../../shared/utils/errors'

const DB_LOCATION_QUERY_KEY = ['db-location']

const useDbLocationQuery = () => {
  return useQuery({ queryKey: DB_LOCATION_QUERY_KEY, queryFn: getDbLocation })
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
  closeModal: () => void
  onAfterDbChange: () => Promise<void>
}

export const useDbLocation = ({ setModalState, closeModal, onAfterDbChange }: UseDbLocationOptions) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { data: info } = useDbLocationQuery()
  const [actionLoading, setActionLoading] = useState(false)

  // onAfterDbChange clears the cache, so the location is fetched again into it for every consumer.
  const afterDbChange = async () => {
    await onAfterDbChange()
    await queryClient.prefetchQuery({ queryKey: DB_LOCATION_QUERY_KEY, queryFn: getDbLocation, staleTime: 0 })
  }

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
      await afterDbChange()
      toast.success(t('dataStorage.toasts.switched', { path: folder }))
      closeModal()
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
      await afterDbChange()
      if (isReset) {
        toast.success(t('dataStorage.toasts.reset'))
      } else if (action === 'move') {
        toast.success(t('dataStorage.toasts.moved', { path: folder }))
      } else {
        toast.success(t('dataStorage.toasts.fresh', { path: folder }))
      }
      closeModal()
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
      await afterDbChange()
      toast.success(t('dataStorage.toasts.reset'))
      closeModal()
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
