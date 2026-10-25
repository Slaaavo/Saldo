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
  deleteSplitGroup,
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
import { LIVE } from '../shared/utils/snapshot'

interface UseModalActionsOptions {
  closeModal: () => void
  onFxRatePrompt: (date: string) => void
}

export const useModalActions = ({ closeModal, onFxRatePrompt }: UseModalActionsOptions) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const snapshotQuery = useSnapshotQuery(LIVE)
  const snapshot = snapshotQuery.data?.rows ?? []
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const consolidationCurrency = consolidationCurrencyQuery.data ?? null

  // Prompt to fetch FX rates when a balance update was saved for a non-consolidation-currency account. The save has already
  // succeeded, so a failed rate lookup must not surface as a failed save.
  const promptForMissingFxRate = async (accountId: number, eventDate: string) => {
    const account = snapshot.find((r) => r.accountId === accountId)
    if (!account || !consolidationCurrency || account.currencyCode === consolidationCurrency.code) return
    // FX rates are stored per calendar day, while a balance update carries a time.
    const rateDate = eventDate.slice(0, 10)
    try {
      const rates = await listFxRates(rateDate)
      if (!rates.some((r) => r.toCurrencyCode === account.currencyCode)) {
        onFxRatePrompt(rateDate)
      }
    } catch (err) {
      console.error('Failed to check FX rates:', err)
    }
  }

  // The balance-update handlers let a failed save reach the modal, which reports it and re-enables its Save button.
  const handleCreateBalanceUpdate = async (accountId: number, amountMinor: number, eventDate: string, note: string): Promise<void> => {
    await createBalanceUpdate(accountId, amountMinor, eventDate, note || undefined)
    closeModal()
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
    await promptForMissingFxRate(accountId, eventDate)
  }

  const handleEditBalanceUpdate = async (eventId: number, amountMinor: number, eventDate: string, note: string): Promise<void> => {
    await updateEvent(eventId, amountMinor, eventDate, note || undefined)
    closeModal()
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
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

  const handleDeleteSplitGroup = async (splitGroupId: number) => {
    try {
      await deleteSplitGroup(splitGroupId)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.deleteSplitGroup', { error: extractErrorMessage(err) }))
    }
  }

  const handleCreateAccount = async (
    name: string,
    currencyId: number,
    initialBalanceMinor?: number,
    accountType?: string,
    linkedAssetIds?: number[],
    iban?: string,
    personId?: number,
  ) => {
    try {
      await createAccount(name, currencyId, initialBalanceMinor, accountType, undefined, linkedAssetIds, iban, personId)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.createAccount', { error: extractErrorMessage(err) }))
    }
  }

  const handleEditAccount = async (
    accountId: number,
    name: string,
    iban?: string,
    personId?: number,
    purchasePriceMinor?: number | null,
    purchaseDate?: string | null,
    depreciationPeriodMonths?: number | null,
  ) => {
    try {
      await updateAccount(accountId, name, iban, personId, purchasePriceMinor, purchaseDate, depreciationPeriodMonths)
      closeModal()
      await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
      await queryClient.invalidateQueries({ queryKey: ['events'] })
    } catch (err) {
      toast.error(t('errors.updateAccount', { error: extractErrorMessage(err) }))
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
      toast.error(t('errors.setAccountAssetLinks', { error: extractErrorMessage(err) }))
    }
  }

  const handleCreateAssetSuccess = async () => {
    closeModal()
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
  }

  const handleCreateBucketBalanceUpdate = async (accountId: number, amountMinor: number, eventDate: string, note: string | null, linkedAccountIds: number[]): Promise<void> => {
    await createBucketBalanceUpdate(accountId, amountMinor, eventDate, note, linkedAccountIds)
    // Close before the FX prompt opens: closing afterwards (as the modal used to) would close the prompt too.
    closeModal()
    await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
    await queryClient.invalidateQueries({ queryKey: ['events'] })
    await promptForMissingFxRate(accountId, eventDate)
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
    handleDeleteSplitGroup,
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
