import { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import type { Currency } from '../../shared/types'
import { listCurrencies, getConsolidationCurrency, setConsolidationCurrency, getAppSetting, setAppSetting } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'

const SAVED_FLASH_MS = 2000

export const useSettings = () => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [currencies, setCurrencies] = useState<Currency[]>([])
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null)
  const [apiKey, setApiKey] = useState('')
  const [apiKeySaved, setApiKeySaved] = useState(false)
  const [currencySaved, setCurrencySaved] = useState(false)

  useEffect(() => {
    Promise.all([listCurrencies(), getConsolidationCurrency(), getAppSetting('oxr_app_id')])
      .then(([all, consolidation, storedKey]) => {
        setCurrencies(all)
        setSelectedCurrency(consolidation)
        if (storedKey) setApiKey(storedKey)
      })
      .catch((err) => console.error('Failed to load settings:', err))
  }, [])

  const flashSaved = useCallback((setter: React.Dispatch<React.SetStateAction<boolean>>) => {
    setter(true)
    setTimeout(() => setter(false), SAVED_FLASH_MS)
  }, [])

  const handleCurrencySelect = useCallback(
    async (currency: Currency) => {
      const previous = selectedCurrency
      setSelectedCurrency(currency)
      try {
        await setConsolidationCurrency(currency.id)
        await queryClient.invalidateQueries({ queryKey: ['consolidation-currency'] })
        await queryClient.invalidateQueries({ queryKey: ['snapshot'] })
        flashSaved(setCurrencySaved)
      } catch (err) {
        setSelectedCurrency(previous)
        toast.error(t('settings.saveCurrencyError', { error: extractErrorMessage(err) }))
      }
    },
    [selectedCurrency, queryClient, flashSaved, t],
  )

  const handleSaveApiKey = useCallback(async () => {
    try {
      await setAppSetting('oxr_app_id', apiKey.trim())
      flashSaved(setApiKeySaved)
    } catch (err) {
      toast.error(t('settings.saveApiKeyError', { error: extractErrorMessage(err) }))
    }
  }, [apiKey, flashSaved, t])

  return {
    currencies,
    selectedCurrency,
    apiKey,
    setApiKey,
    apiKeySaved,
    currencySaved,
    handleCurrencySelect,
    handleSaveApiKey,
  }
}
