import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { Currency } from '../types';
import {
  listCurrencies,
  getConsolidationCurrency,
  setConsolidationCurrency,
  getAppSetting,
  setAppSetting,
} from '../api';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';

interface Props {
  onClose: () => void;
  onConsolidationCurrencyChange: () => void;
}

export default function SettingsPanel({ onClose, onConsolidationCurrencyChange }: Props) {
  const { t } = useTranslation();
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null);
  const [currencySearch, setCurrencySearch] = useState('');
  const [showDropdown, setShowDropdown] = useState(false);
  const [apiKey, setApiKey] = useState('');
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    Promise.all([listCurrencies(), getConsolidationCurrency(), getAppSetting('oxr_app_id')])
      .then(([all, consolidation, storedKey]) => {
        setCurrencies(all);
        setSelectedCurrency(consolidation);
        if (storedKey) setApiKey(storedKey);
      })
      .catch((err) => console.error('Failed to load settings:', err));
  }, []);

  const filteredCurrencies = currencies.filter(
    (c) =>
      c.code.toLowerCase().includes(currencySearch.toLowerCase()) ||
      c.name.toLowerCase().includes(currencySearch.toLowerCase()),
  );

  const handleCurrencySelect = async (currency: Currency) => {
    setSelectedCurrency(currency);
    setCurrencySearch('');
    setShowDropdown(false);
    try {
      await setConsolidationCurrency(currency.id);
      onConsolidationCurrencyChange();
    } catch (err) {
      console.error('Failed to set consolidation currency:', err);
    }
  };

  const handleCurrencyInputFocus = () => {
    setCurrencySearch('');
    setShowDropdown(true);
  };

  const handleCurrencyInputBlur = () => {
    setTimeout(() => setShowDropdown(false), 150);
  };

  const handleSaveApiKey = async () => {
    try {
      await setAppSetting('oxr_app_id', apiKey.trim());
      setApiKeySaved(true);
      setTimeout(() => setApiKeySaved(false), 2000);
    } catch (err) {
      console.error('Failed to save API key:', err);
    }
  };

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('settings.title')}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-6">
          {/* Consolidation Currency */}
          <div className="flex flex-col gap-2">
            <Label>{t('settings.consolidationCurrency')}</Label>
            <div className="relative" ref={dropdownRef}>
              <Input
                type="text"
                value={
                  showDropdown
                    ? currencySearch
                    : selectedCurrency
                      ? `${selectedCurrency.code} — ${selectedCurrency.name}`
                      : ''
                }
                onChange={(e) => setCurrencySearch(e.target.value)}
                onFocus={handleCurrencyInputFocus}
                onBlur={handleCurrencyInputBlur}
                placeholder={t('currency.searchPlaceholder')}
              />
              {showDropdown && (
                <div className="absolute z-50 mt-1 w-full max-h-48 overflow-y-auto rounded-md border border-border bg-popover shadow-md">
                  {filteredCurrencies.length === 0 ? (
                    <div className="px-3 py-2 text-sm text-muted-foreground">
                      {t('currency.noResults')}
                    </div>
                  ) : (
                    filteredCurrencies.map((c) => (
                      <button
                        key={c.id}
                        type="button"
                        className="w-full px-3 py-2 text-left text-sm hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground outline-none"
                        onMouseDown={() => handleCurrencySelect(c)}
                      >
                        <span className="font-medium">{c.code}</span>
                        <span className="ml-2 text-muted-foreground">{c.name}</span>
                      </button>
                    ))
                  )}
                </div>
              )}
            </div>
          </div>

          {/* OXR API Key */}
          <div className="flex flex-col gap-2">
            <Label htmlFor="settings-oxr-key">{t('settings.oxrApiKey')}</Label>
            <div className="flex gap-2">
              <Input
                id="settings-oxr-key"
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={t('settings.oxrApiKeyPlaceholder')}
                className="flex-1"
              />
              <Button
                type="button"
                onClick={handleSaveApiKey}
                variant={apiKeySaved ? 'default' : 'outline'}
              >
                {apiKeySaved ? t('settings.saved') : t('settings.saveApiKey')}
              </Button>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
