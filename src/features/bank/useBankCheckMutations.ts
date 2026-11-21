import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { bookUnexplainedDifference, confirmBankMatch, deleteBalanceCheck, deleteEvent, deleteSplitGroup, updateEvent } from '@/shared/api'
import type { DifferenceCause, RowTarget } from '@/shared/types'
import { extractErrorMessage, hasErrorCode } from '@/shared/utils/errors'

interface MatchVariables {
  accountId: number
  seenBalanceMinor: number
}

interface BookVariables extends MatchVariables {
  bankBalanceMinor: number
}

export type RowCause = Exclude<DifferenceCause, { kind: 'staleImport' }>

const useRefreshAfterCheck = () => {
  const queryClient = useQueryClient()
  return async () => {
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
    await queryClient.invalidateQueries({ queryKey: ['bank-check-causes'] })
  }
}

const deleteRow = (target: RowTarget) => (target.kind === 'splitGroup' ? deleteSplitGroup(target.splitGroupId) : deleteEvent(target.eventId))

const fixCause = (cause: RowCause): Promise<void> => {
  if (cause.kind === 'flippedSign') {
    const { target, amountMinor, eventDate, note } = cause.row
    return updateEvent(target.eventId, -amountMinor, eventDate, note ?? undefined)
  }
  return deleteRow(cause.row.target)
}

export const useFixDifferenceCause = () => {
  const { t } = useTranslation()
  const refresh = useRefreshAfterCheck()

  return useMutation({
    mutationFn: fixCause,
    onSuccess: async (_, cause) => {
      toast.success(t(cause.kind === 'flippedSign' ? 'bankCheck.causes.flipped' : 'bankCheck.causes.deleted'))
      await refresh()
    },
    onError: async (err) => {
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
    mutationFn: ({ accountId, seenBalanceMinor }: MatchVariables) => confirmBankMatch(accountId, seenBalanceMinor),
    onSuccess: async (checkId) => {
      toast.success(t('bankCheck.matched'), { action: { label: t('bankCheck.undo'), onClick: () => void undo(checkId) } })
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
