import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { Currency } from '../types';
import type { ThemePreference } from '../hooks/useTheme';
import {
  listCurrencies,
  getConsolidationCurrency,
  setConsolidationCurrency,
  getAppSetting,
  setAppSetting,
} from '../api';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import CurrencySelect from './CurrencySelect';
import LanguageSelector from './LanguageSelector';

interface Props {
  onConsolidationCurrencyChange: () => void;
  themePreference: ThemePreference;
  onThemeChange: (pref: ThemePreference) => void;
}

export default function SettingsPage({
  onConsolidationCurrencyChange,
  themePreference,
  onThemeChange,
}: Props) {
  const { t } = useTranslation();
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

  const handleCurrencySelect = async (currency: Currency) => {
    setSelectedCurrency(currency);
    try {
      await setConsolidationCurrency(currency.id);
      onConsolidationCurrencyChange();
      setCurrencySaved(true);
      setTimeout(() => setCurrencySaved(false), 2000);
    } catch (err) {
      console.error('Failed to set consolidation currency:', err);
    }
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
    <div className="px-4 md:px-10 py-8">
      {/* Section 1: Display */}
      <div className="border-b border-border pb-8 mb-8">
        <h3 className="text-lg font-semibold mb-1">{t('settings.sectionDisplay')}</h3>
        <p className="text-sm text-muted-foreground mb-6">{t('settings.sectionDisplayDesc')}</p>

        {/* Language field */}
        <div className="flex flex-col gap-2 mb-4">
          <Label>{t('settings.language')}</Label>
          <LanguageSelector />
        </div>

        {/* Theme field */}
        <div className="flex flex-col gap-2 mb-4">
          <Label>{t('settings.theme.label')}</Label>
          <Select value={themePreference} onValueChange={onThemeChange}>
            <SelectTrigger className="w-64 h-10 text-sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="light">{t('settings.theme.light')}</SelectItem>
              <SelectItem value="dark">{t('settings.theme.dark')}</SelectItem>
              <SelectItem value="system">{t('settings.theme.system')}</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Consolidation Currency field */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-3">
            <Label>{t('settings.consolidationCurrency')}</Label>
            {currencySaved && (
              <span className="text-sm text-green-600 dark:text-green-400">
                {t('settings.saved')}
              </span>
            )}
          </div>
          <CurrencySelect
            currencies={currencies}
            value={selectedCurrency}
            onChange={handleCurrencySelect}
            pinnedCurrencyCodes={['EUR', 'USD', 'BTC']}
            className="w-64"
          />
        </div>
      </div>

      {/* Section 2: Integrations */}
      <div>
        <h3 className="text-lg font-semibold mb-1">{t('settings.sectionIntegrations')}</h3>
        <p className="text-sm text-muted-foreground mb-6">
          {t('settings.sectionIntegrationsDesc')}
        </p>

        {/* OXR API Key field */}
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
    </div>
  );
}
