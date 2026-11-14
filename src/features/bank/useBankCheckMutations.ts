import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { bookUnexplainedDifference, confirmBankMatch, deleteBalanceCheck } from '@/shared/api'
import { extractErrorMessage, hasErrorCode } from '@/shared/utils/errors'

interface MatchVariables {
  accountId: number
  /** The live balance the user looked at. The backend refuses with `CHECK_CHANGED` when it moved since. */
  seenBalanceMinor: number
}

interface BookVariables extends MatchVariables {
  bankBalanceMinor: number
}

const useRefreshAfterCheck = () => {
  const queryClient = useQueryClient()
  return async () => {
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
  }
}

/** Records that the bank app shows Saldo's live balance. The toast offers Undo, which deletes the check. */
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

/** Books the bank's balance minus Saldo's as one unexplained difference, which also records a matching check. */
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
      if (hasErrorCode(err, 'CHECK_CHANGED') || hasErrorCode(err, 'NOTHING_TO_BOOK')) {
        toast.error(t(hasErrorCode(err, 'CHECK_CHANGED') ? 'bankCheck.changed' : 'bankCheck.dialog.nothingToBook'))
        await refresh()
        return
      }
      toast.error(t('bankCheck.dialog.failed', { error: extractErrorMessage(err) }))
    },
  })
}
