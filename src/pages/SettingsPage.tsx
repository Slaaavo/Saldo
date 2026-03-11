import { useTranslation } from 'react-i18next';
import { PINNED_CURRENCY_CODES } from '../config/constants';
import type { ThemePreference } from '../hooks/useTheme';
import { useSettings } from '../hooks/useSettings';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../components/ui/select';
import CurrencySelect from '../components/CurrencySelect';
import LanguageSelector from '../components/LanguageSelector';

interface Props {
  onConsolidationCurrencyChange: () => void;
  themePreference: ThemePreference;
  onThemeChange: (pref: ThemePreference) => void;
  isDemoMode: boolean;
  onEnterDemoMode: () => void;
  onExitDemoMode: () => void;
  dbLocationPath: string;
  dbLocationIsDefault: boolean;
  onChangeDbLocation: () => void;
  onResetDbLocation: () => void;
}

export default function SettingsPage({
  onConsolidationCurrencyChange,
  themePreference,
  onThemeChange,
  isDemoMode,
  onEnterDemoMode,
  onExitDemoMode,
  dbLocationPath,
  dbLocationIsDefault,
  onChangeDbLocation,
  onResetDbLocation,
}: Props) {
  const { t } = useTranslation();
  const {
    currencies,
    selectedCurrency,
    apiKey,
    setApiKey,
    apiKeySaved,
    currencySaved,
    handleCurrencySelect,
    handleSaveApiKey,
  } = useSettings({ onConsolidationCurrencyChange });

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
            pinnedCurrencyCodes={PINNED_CURRENCY_CODES}
            className="w-64"
          />
        </div>
      </div>

      {/* Section 2: Integrations */}
      <div className="border-b border-border pb-8 mb-8">
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

      {/* Section 3: Data Storage */}
      <div className="border-b border-border pb-8 mb-8">
        <h3 className="text-lg font-semibold mb-1">{t('dataStorage.sectionTitle')}</h3>
        <p className="text-sm text-muted-foreground mb-6">{t('dataStorage.sectionDesc')}</p>

        <div className="flex flex-col gap-3">
          <Label>{t('dataStorage.currentPath')}</Label>
          <p className="select-text break-all rounded-md bg-muted px-3 py-2 font-mono text-sm">
            {dbLocationPath}
            {dbLocationIsDefault && (
              <span className="ml-2 text-xs text-muted-foreground">
                {t('dataStorage.isDefault')}
              </span>
            )}
          </p>

          {isDemoMode && (
            <p className="text-sm text-muted-foreground">{t('dataStorage.disabledInDemoMode')}</p>
          )}

          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={onChangeDbLocation}
              disabled={isDemoMode}
            >
              {t('dataStorage.changeButton')}
            </Button>
            {!dbLocationIsDefault && (
              <Button
                type="button"
                variant="outline"
                onClick={onResetDbLocation}
                disabled={isDemoMode}
              >
                {t('dataStorage.resetButton')}
              </Button>
            )}
          </div>
        </div>
      </div>

      {/* Section 4: Demo Mode */}
      <div>
        <h3 className="text-lg font-semibold mb-1">{t('demo.settingsTitle')}</h3>
        <p className="text-sm text-muted-foreground mb-6">{t('demo.settingsDesc')}</p>
        {isDemoMode ? (
          <Button variant="destructive" onClick={onExitDemoMode}>
            {t('demo.stopButton')}
          </Button>
        ) : (
          <div className="flex flex-col gap-4">
            <p className="text-sm text-muted-foreground">{t('demo.settingsNote')}</p>
            <div>
              <Button onClick={onEnterDemoMode}>{t('demo.startButton')}</Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
