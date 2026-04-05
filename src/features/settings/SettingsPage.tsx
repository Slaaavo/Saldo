import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { PINNED_CURRENCY_CODES } from '../../shared/config/constants'
import { useSettings } from './useSettings'
import { useDemo } from '../../app/DemoContext'
import { useTheme } from './useTheme'
import { useModal } from '../../app/ModalContext'
import { useDbLocation } from './useDbLocation'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'
import CurrencySelect from '../currency/CurrencySelect'
import LanguageSelector from './LanguageSelector'
import BulkUpdateVisibilityModal from './BulkUpdateVisibilityModal'
import { setBulkUpdateExclusions } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import { useSnapshotQuery } from '../../shared/hooks/useSnapshotQuery'
import { useBulkUpdateExclusionsQuery } from '../../shared/hooks/useBulkUpdateExclusionsQuery'
import { todayIso } from '../../shared/utils/format'

export default function SettingsPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [visibilityModalOpen, setVisibilityModalOpen] = useState(false)
  const snapshotQuery = useSnapshotQuery(todayIso())
  const snapshot = snapshotQuery.data ?? []
  const exclusionsQuery = useBulkUpdateExclusionsQuery()
  const exclusions = exclusionsQuery.data ?? []
  const { currencies, selectedCurrency, apiKey, setApiKey, apiKeySaved, currencySaved, handleCurrencySelect, handleSaveApiKey } = useSettings()
  const { isDemoMode, onEnterDemoMode, onExitDemoMode } = useDemo()
  const { themePreference, setThemePreference } = useTheme()
  const { setModalState, closeModal } = useModal()
  const dbLocation = useDbLocation({ setModalState, closeModal, onAfterDbChange: async () => {} })

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
          <Select value={themePreference} onValueChange={setThemePreference}>
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
            {currencySaved && <span className="text-sm text-green-600 dark:text-green-400">{t('settings.saved')}</span>}
          </div>
          <CurrencySelect currencies={currencies} value={selectedCurrency} onChange={handleCurrencySelect} pinnedCurrencyCodes={PINNED_CURRENCY_CODES} className="w-64" />
        </div>
      </div>

      {/* Section 2: Integrations */}
      <div className="border-b border-border pb-8 mb-8">
        <h3 className="text-lg font-semibold mb-1">{t('settings.sectionIntegrations')}</h3>
        <p className="text-sm text-muted-foreground mb-6">{t('settings.sectionIntegrationsDesc')}</p>

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
            <Button type="button" onClick={handleSaveApiKey} variant={apiKeySaved ? 'default' : 'outline'}>
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
            {dbLocation.path}
            {dbLocation.isDefault && <span className="ml-2 text-xs text-muted-foreground">{t('dataStorage.isDefault')}</span>}
          </p>

          {isDemoMode && <p className="text-sm text-muted-foreground">{t('dataStorage.disabledInDemoMode')}</p>}

          <div className="flex gap-2">
            <Button type="button" variant="outline" onClick={dbLocation.handleChange} disabled={isDemoMode}>
              {t('dataStorage.changeButton')}
            </Button>
            {!dbLocation.isDefault && (
              <Button type="button" variant="outline" onClick={dbLocation.handleReset} disabled={isDemoMode}>
                {t('dataStorage.resetButton')}
              </Button>
            )}
          </div>
        </div>
      </div>

      {/* Section 4: Bulk Update Visibility */}
      <div className="border-b border-border pb-8 mb-8">
        <h3 className="text-lg font-semibold mb-1">{t('settings.bulkUpdateVisibility.title')}</h3>
        <p className="text-sm text-muted-foreground mb-6">{t('settings.bulkUpdateVisibility.desc')}</p>
        <div className="flex flex-col gap-3">
          <p className="text-sm text-muted-foreground">
            {exclusions.length === 0
              ? t('settings.bulkUpdateVisibility.allIncluded')
              : (() => {
                  const exclusionSet = new Set(exclusions)
                  return snapshot
                    .filter((r) => !exclusionSet.has(r.accountId))
                    .map((r) => r.accountName)
                    .join(', ')
                })()}
          </p>
          <div>
            <Button type="button" variant="outline" onClick={() => setVisibilityModalOpen(true)}>
              {t('settings.bulkUpdateVisibility.edit')}
            </Button>
          </div>
        </div>
      </div>

      {visibilityModalOpen && (
        <BulkUpdateVisibilityModal
          accounts={snapshot}
          currentExclusions={exclusions}
          onSave={async (excludedIds) => {
            try {
              await setBulkUpdateExclusions(excludedIds)
              await queryClient.invalidateQueries({ queryKey: ['bulk-update-exclusions'] })
              setVisibilityModalOpen(false)
            } catch (err) {
              toast.error(t('errors.bulkUpdateVisibility', { error: extractErrorMessage(err) }))
            }
          }}
          onClose={() => setVisibilityModalOpen(false)}
        />
      )}

      {/* Section 5: Demo Mode */}
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
  )
}
