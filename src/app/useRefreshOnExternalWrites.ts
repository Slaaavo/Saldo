import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getDbDataVersion } from '../shared/api'

/**
 * Shows what another process (`saldo mcp`) wrote while the window was in the background. Queries
 * never go stale on their own, so when the window comes back and the database's data version
 * moved, every query is invalidated. The app's own writes never move it.
 *
 * The first check is the baseline. A database swap reloads the window, which mounts this hook
 * again, so versions of two databases are never compared.
 */
export const useRefreshOnExternalWrites = () => {
  const queryClient = useQueryClient()
  const { t } = useTranslation()

  useEffect(() => {
    let lastSeen: number | undefined
    let checking = false
    let disposed = false
    let stopFocusListener: (() => void) | undefined

    const check = async () => {
      if (checking) return
      checking = true
      try {
        const version = await getDbDataVersion()
        const changed = lastSeen !== undefined && version !== lastSeen
        lastSeen = version
        if (changed) {
          await queryClient.invalidateQueries()
          toast.info(t('dataStorage.toasts.externalChanges'))
        }
      } catch (err) {
        // A failed check must not toast on every focus; the next focus checks again.
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
