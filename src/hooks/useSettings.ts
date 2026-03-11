import { useState, useEffect, useCallback } from 'react';
import type { Currency } from '../types';
import {
  listCurrencies,
  getConsolidationCurrency,
  setConsolidationCurrency,
  getAppSetting,
  setAppSetting,
} from '../api';

const SAVED_FLASH_MS = 2000;

interface UseSettingsOptions {
  onConsolidationCurrencyChange: () => void;
}

export function useSettings({ onConsolidationCurrencyChange }: UseSettingsOptions) {
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [currencySaved, setCurrencySaved] = useState(false);

  useEffect(() => {
    Promise.all([listCurrencies(), getConsolidationCurrency(), getAppSetting('oxr_app_id')])
      .then(([all, consolidation, storedKey]) => {
        setCurrencies(all);
        setSelectedCurrency(consolidation);
        if (storedKey) setApiKey(storedKey);
      })
      .catch((err) => console.error('Failed to load settings:', err));
  }, []);

  const flashSaved = useCallback((setter: React.Dispatch<React.SetStateAction<boolean>>) => {
    setter(true);
    setTimeout(() => setter(false), SAVED_FLASH_MS);
  }, []);

  const handleCurrencySelect = useCallback(
    async (currency: Currency) => {
      setSelectedCurrency(currency);
      try {
        await setConsolidationCurrency(currency.id);
        onConsolidationCurrencyChange();
        flashSaved(setCurrencySaved);
      } catch (err) {
        console.error('Failed to set consolidation currency:', err);
      }
    },
    [onConsolidationCurrencyChange, flashSaved],
  );

  const handleSaveApiKey = useCallback(async () => {
    try {
      await setAppSetting('oxr_app_id', apiKey.trim());
      flashSaved(setApiKeySaved);
    } catch (err) {
      console.error('Failed to save API key:', err);
    }
  }, [apiKey, flashSaved]);

  return {
    currencies,
    selectedCurrency,
    apiKey,
    setApiKey,
    apiKeySaved,
    currencySaved,
    handleCurrencySelect,
    handleSaveApiKey,
  };
}
