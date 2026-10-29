import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getDbDataVersion } from '../shared/api'

// A DB swap reloads the window and remounts the hook, so versions of two databases are never compared.
export const useRefreshOnExternalWrites = () => {
  const queryClient = useQueryClient()
  const { t } = useTranslation()

  useEffect(() => {
    let baselineVersion: number | undefined
    let checking = false
    let disposed = false
    let stopFocusListener: (() => void) | undefined

    const check = async () => {
      if (checking) return
      checking = true
      try {
        const version = await getDbDataVersion()
        const changed = baselineVersion !== undefined && version !== baselineVersion
        baselineVersion = version
        if (changed) {
          await queryClient.invalidateQueries()
          toast.info(t('dataStorage.toasts.externalChanges'))
        }
      } catch (err) {
        console.warn('Could not check for changes made outside Saldo', err)
      } finally {
        checking = false
      }
    }

    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') void check()
    }

    const listenForFocus = async () => {
      try {
        const stop = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (focused) void check()
        })
        if (disposed) stop()
        else stopFocusListener = stop
      } catch (err) {
        console.warn('Could not listen for window focus', err)
      }
    }

    void check()
    document.addEventListener('visibilitychange', onVisibilityChange)
    void listenForFocus()
    return () => {
      disposed = true
      document.removeEventListener('visibilitychange', onVisibilityChange)
      stopFocusListener?.()
    }
  }, [queryClient, t])
}
