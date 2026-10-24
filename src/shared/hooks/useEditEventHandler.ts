import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EventWithData } from '../types'
import { getEventById } from '../api'
import { extractErrorMessage } from '../utils/errors'
import { useModal } from '../../app/useModal'

/** Opens the edit modal matching the event type. Adding an editable event type means adding a branch here. */
export const useEditEventHandler = () => {
  const { t } = useTranslation()
  const { setModalState } = useModal()

  return async (event: EventWithData) => {
    if (event.eventType === 'balance_update') {
      setModalState({ type: 'editBalanceUpdate', event })
      return
    }
    if (event.eventType === 'revenue' || event.eventType === 'expense' || event.eventType === 'prepaid_expense') {
      setModalState({ type: 'editTaxableEvent', event })
      return
    }
    if (event.eventType === 'transfer') {
      if (!event.linkedEventId) {
        toast.error(t('errors.linkedEventMissing'))
        return
      }
      try {
        const linked = await getEventById(event.linkedEventId)
        if (!linked) {
          toast.error(t('errors.linkedEventMissing'))
          return
        }
        const fromEvent = event.amountMinor < 0 ? event : linked
        const toEvent = event.amountMinor < 0 ? linked : event
        setModalState({ type: 'editTransfer', fromEvent, toEvent })
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    }
  }
}
