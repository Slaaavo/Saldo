import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useAskToBreakChecks } from '@/app/useAskToBreakChecks'
import { bookUnexplainedDifference, ChangeDeclined, confirmBankMatch, deleteBalanceCheck, deleteEvent, deleteSplitGroup, updateEvent, type AskToBreakChecks } from '@/shared/api'
import type { DifferenceCause, RowTarget } from '@/shared/types'
import { extractErrorMessage, hasErrorCode } from '@/shared/utils/errors'

interface SeenVariables {
  accountId: number
  seenBalanceMinor: number
}

interface MatchVariables extends SeenVariables {
  notArrivedEventIds?: number[]
}

interface BookVariables extends SeenVariables {
  bankBalanceMinor: number
}

export type RowCause = Exclude<DifferenceCause, { kind: 'staleImport' | 'notArrived' }>

const useRefreshAfterCheck = () => {
  const queryClient = useQueryClient()
  return async () => {
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
    await queryClient.invalidateQueries({ queryKey: ['bank-check-causes'] })
  }
}

const deleteRow = (target: RowTarget, ask: AskToBreakChecks) => (target.kind === 'splitGroup' ? deleteSplitGroup(target.splitGroupId, ask) : deleteEvent(target.eventId, ask))

const fixCause = (cause: RowCause, ask: AskToBreakChecks): Promise<void> => {
  if (cause.kind === 'flippedSign') {
    const { target, amountMinor, eventDate, note } = cause.row
    return updateEvent(target.eventId, -amountMinor, eventDate, note, ask)
  }
  return deleteRow(cause.row.target, ask)
}

export const useFixDifferenceCause = () => {
  const { t } = useTranslation()
  const refresh = useRefreshAfterCheck()
  const ask = useAskToBreakChecks()

  return useMutation({
    mutationFn: (cause: RowCause) => fixCause(cause, ask),
    onSuccess: async (_, cause) => {
      toast.success(t(cause.kind === 'flippedSign' ? 'bankCheck.causes.flipped' : 'bankCheck.causes.deleted'))
      await refresh()
    },
    onError: async (err) => {
      if (err instanceof ChangeDeclined) return
      toast.error(t('bankCheck.causes.fixFailed', { error: extractErrorMessage(err) }))
      await refresh()
    },
  })
}

export const useConfirmBankMatch = () => {
  const { t } = useTranslation()
  const refresh = useRefreshAfterCheck()

  const undo = async (checkId: number) => {
    try {
      await deleteBalanceCheck(checkId)
      await refresh()
    } catch (err) {
      toast.error(t('bankCheck.undoFailed', { error: extractErrorMessage(err) }))
    }
  }

  return useMutation({
    mutationFn: ({ accountId, seenBalanceMinor, notArrivedEventIds = [] }: MatchVariables) => confirmBankMatch(accountId, seenBalanceMinor, notArrivedEventIds),
    onSuccess: async (checkId, { notArrivedEventIds }) => {
      toast.success(t(notArrivedEventIds?.length ? 'bankCheck.matchedNotArrived' : 'bankCheck.matched'), {
        action: { label: t('bankCheck.undo'), onClick: () => void undo(checkId) },
      })
      await refresh()
    },
    onError: async (err) => {
      if (hasErrorCode(err, 'CHECK_CHANGED')) {
        toast.error(t('bankCheck.changed'))
        await refresh()
        return
      }
      toast.error(t('bankCheck.failed', { error: extractErrorMessage(err) }))
    },
  })
}

export const useBookUnexplainedDifference = () => {
  const { t } = useTranslation()
  const refresh = useRefreshAfterCheck()

  return useMutation({
    mutationFn: ({ accountId, bankBalanceMinor, seenBalanceMinor }: BookVariables) => bookUnexplainedDifference(accountId, bankBalanceMinor, seenBalanceMinor),
    onSuccess: async () => {
      toast.success(t('bankCheck.dialog.booked'))
      await refresh()
    },
    onError: async (err) => {
      if (hasErrorCode(err, 'CHECK_CHANGED')) {
        toast.error(t('bankCheck.changed'))
        await refresh()
        return
      }
      toast.error(t('bankCheck.dialog.failed', { error: extractErrorMessage(err) }))
    },
  })
}
