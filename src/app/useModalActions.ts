import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import {
  createBalanceUpdate,
  createAccount,
  updateAccount,
  deleteAccount,
  updateEvent,
  deleteEvent,
  bulkCreateBalanceUpdates,
  listFxRates,
  updateSortOrder,
  updateAssetValue,
  setAccountAssetLinks,
  createBucketBalanceUpdate,
  updateBucketBalanceUpdate,
  updateTransfer,
} from '../shared/api'
import { extractErrorMessage } from '../shared/utils/errors'
import { useSnapshotQuery } from '../shared/hooks/useSnapshotQuery'
import { useConsolidationCurrencyQuery } from '../shared/hooks/useConsolidationCurrencyQuery'
import { todayIso } from '../shared/utils/format'

interface UseModalActionsOptions {
  closeModal: () => void
  onFxRatePrompt: (date: string) => void
}

export function useModalActions({ closeModal, onFxRatePrompt }: UseModalActionsOptions) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const snapshotQuery = useSnapshotQuery(todayIso())
  const snapshot = snapshotQuery.data ?? []
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const consolidationCurrency = consolidationCurrencyQuery.data ?? null

  const handleCreateBalanceUpdate = async (accountId: number, amountMinor: number, eventDate: string, note: string) => {
    const account = snapshot.find((r) => r.accountId === accountId)
    try {
      await createBalanceUpdate(accountId, amountMinor, eventDate, note || undefined)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
      // Prompt to fetch FX rates when saving a non-consolidation-currency balance update
      if (account && consolidationCurrency && account.currencyCode !== consolidationCurrency.code) {
        const rates = await listFxRates(eventDate)
        const hasRate = rates.some((r) => r.toCurrencyCode === account.currencyCode)
        if (!hasRate) {
          onFxRatePrompt(eventDate)
        }
      }
    } catch (err) {
      toast.error(t('errors.createBalanceUpdate', { error: extractErrorMessage(err) }))
    }
  }

  const handleEditBalanceUpdate = async (eventId: number, amountMinor: number, eventDate: string, note: string) => {
    try {
      await updateEvent(eventId, amountMinor, eventDate, note || undefined)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.updateEvent', { error: extractErrorMessage(err) }))
    }
  }

  const handleDeleteEvent = async (eventId: number) => {
    try {
      await deleteEvent(eventId)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.deleteEvent', { error: extractErrorMessage(err) }))
    }
  }

  const handleCreateAccount = async (name: string, currencyId: number, initialBalanceMinor?: number, accountType?: string, linkedAssetIds?: number[], iban?: string) => {
    try {
      await createAccount(name, currencyId, initialBalanceMinor, accountType, undefined, linkedAssetIds, iban)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.createAccount', { error: extractErrorMessage(err) }))
    }
  }

  const handleEditAccount = async (accountId: number, name: string, iban?: string) => {
    try {
      await updateAccount(accountId, name, iban)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.renameAccount', { error: extractErrorMessage(err) }))
    }
  }

  const handleDeleteAccount = async (accountId: number) => {
    try {
      await deleteAccount(accountId)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      const msg = extractErrorMessage(err)
      if (msg.includes('currently linked to buckets')) {
        toast.error(t('errors.deleteAccountLinked'))
      } else {
        toast.error(t('errors.deleteAccount', { error: msg }))
      }
    }
  }

  const handleBulkUpdateSubmit = async (updates: { accountId: number; amountMinor: number }[], eventDate: string, note: string) => {
    await bulkCreateBalanceUpdates(updates, eventDate, note || undefined)
    closeModal()
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
  }

  const handleSaveOrder = async (orderedIds: number[]) => {
    try {
      const entries = orderedIds.map((accountId, index) => ({ accountId, sortOrder: index }))
      await updateSortOrder(entries)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  const handleUpdateAssetValue = async (accountId: number, amountMinor: number | null, pricePerUnit: string | null, eventDate: string, note: string | null) => {
    try {
      await updateAssetValue(accountId, amountMinor, pricePerUnit, eventDate, note)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.updateAssetValue', { error: extractErrorMessage(err) }))
    }
  }

  const handleSetAccountAssetLinks = async (accountId: number, assetIds: number[]) => {
    try {
      await setAccountAssetLinks(accountId, assetIds)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(extractErrorMessage(err))
    }
  }

  const handleCreateAssetSuccess = async () => {
    closeModal()
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
  }

  const handleCreateBucketBalanceUpdate = async (accountId: number, amountMinor: number, eventDate: string, note: string | null, linkedAccountIds: number[]): Promise<void> => {
    const account = snapshot.find((r) => r.accountId === accountId)
    await createBucketBalanceUpdate(accountId, amountMinor, eventDate, note, linkedAccountIds)
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
    if (account && consolidationCurrency && account.currencyCode !== consolidationCurrency.code) {
      const rates = await listFxRates(eventDate)
      const hasRate = rates.some((r) => r.toCurrencyCode === account.currencyCode)
      if (!hasRate) onFxRatePrompt(eventDate)
    }
  }

  const handleEditBucketBalanceUpdate = async (eventId: number, amountMinor: number, eventDate: string, note: string | null, linkedAccountIds: number[]): Promise<void> => {
    await updateBucketBalanceUpdate(eventId, amountMinor, eventDate, note, linkedAccountIds)
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
  }

  const handleEditTransfer = async (payload: {
    fromEventId: number
    toEventId: number
    fromDate: string
    toDate: string
    amountMinor: number
    toAmountMinor: number
    note: string | null
    originalCurrencyId: number | null
    fxRateMantissa: number | null
    fxRateExponent: number | null
  }) => {
    try {
      await updateTransfer(payload)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.updateTransfer', { error: extractErrorMessage(err) }))
    }
  }

  return {
    handleCreateBalanceUpdate,
    handleEditBalanceUpdate,
    handleDeleteEvent,
    handleCreateAccount,
    handleEditAccount,
    handleDeleteAccount,
    handleBulkUpdateSubmit,
    handleSaveOrder,
    handleUpdateAssetValue,
    handleSetAccountAssetLinks,
    handleCreateAssetSuccess,
    handleCreateBucketBalanceUpdate,
    handleEditBucketBalanceUpdate,
    handleEditTransfer,
  }
}
