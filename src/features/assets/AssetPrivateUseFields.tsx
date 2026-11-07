import { useTranslation } from 'react-i18next'
import type { AssetPurchasePreview } from '../../shared/types'
import { getMinorUnitsStep } from '../../shared/utils/format'
import { Checkbox } from '../../shared/ui/checkbox'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Label } from '../../shared/ui/label'
import NumberValue from '../../shared/ui/NumberValue'
import { PercentageInput } from '../../shared/ui/PercentageInput'
import type { AssetUseDraft } from './assetUseDraft'

const DEFAULT_BUSINESS_USE_PCT = '80'
const DEFAULT_VAT_RECLAIMABLE_PCT = '50'

interface Props {
  idPrefix: string
  draft: AssetUseDraft
  onChange: (draft: AssetUseDraft) => void
  showVatFields: boolean
  currencyCode: string
  minorUnits: number
  preview: AssetPurchasePreview | undefined
}

const AssetPrivateUseFields = ({ idPrefix, draft, onChange, showVatFields, currencyCode, minorUnits, preview }: Props) => {
  const { t } = useTranslation()

  const handleSwitch = (privateUse: boolean) => {
    onChange({
      ...draft,
      privateUse,
      businessUsePct: draft.businessUsePct || DEFAULT_BUSINESS_USE_PCT,
      vatReclaimablePct: draft.vatReclaimablePct || DEFAULT_VAT_RECLAIMABLE_PCT,
    })
  }

  const amount = (value: number) => <NumberValue value={value} minorUnits={minorUnits} currencyCode={currencyCode} className="tabular-nums" />

  return (
    <>
      <div className="flex items-center gap-2">
        <Checkbox id={`${idPrefix}-private-use`} checked={draft.privateUse} onCheckedChange={(v) => handleSwitch(v === true)} />
        <Label htmlFor={`${idPrefix}-private-use`}>{t('modals.assetDepreciation.privateUse')}</Label>
      </div>
      {draft.privateUse && (
        <>
          {showVatFields && (
            <>
              <div className="flex flex-col gap-2">
                <Label htmlFor={`${idPrefix}-gross-price`}>{t('modals.assetDepreciation.grossPrice')}</Label>
                <CurrencyInput
                  id={`${idPrefix}-gross-price`}
                  type="number"
                  step={getMinorUnitsStep(minorUnits)}
                  value={draft.grossPrice}
                  onChange={(e) => onChange({ ...draft, grossPrice: e.target.value })}
                  placeholder="0"
                  currencyCode={currencyCode}
                />
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor={`${idPrefix}-vat-rate`}>{t('modals.assetDepreciation.vatRate')}</Label>
                <PercentageInput
                  id={`${idPrefix}-vat-rate`}
                  type="number"
                  step="0.01"
                  min={0}
                  max={100}
                  value={draft.vatRate}
                  onChange={(e) => onChange({ ...draft, vatRate: e.target.value })}
                />
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor={`${idPrefix}-vat-reclaimable`}>{t('modals.assetDepreciation.vatReclaimablePct')}</Label>
                <PercentageInput
                  id={`${idPrefix}-vat-reclaimable`}
                  type="number"
                  step="0.01"
                  min={0}
                  max={100}
                  value={draft.vatReclaimablePct}
                  onChange={(e) => onChange({ ...draft, vatReclaimablePct: e.target.value })}
                />
              </div>
            </>
          )}
          <div className="flex flex-col gap-2">
            <Label htmlFor={`${idPrefix}-business-use`}>{t('modals.assetDepreciation.businessUse')}</Label>
            <PercentageInput
              id={`${idPrefix}-business-use`}
              type="number"
              step="0.01"
              min={0}
              max={100}
              value={draft.businessUsePct}
              onChange={(e) => onChange({ ...draft, businessUsePct: e.target.value })}
            />
          </div>
          {preview && (
            <dl className="grid grid-cols-[auto_auto_1fr] gap-x-3 gap-y-1 text-sm">
              <dt className="text-muted-foreground">{t('modals.assetDepreciation.summaryDepreciates')}</dt>
              <dd className="text-right">{amount(preview.depreciatingMinor)}</dd>
              <dd className="text-muted-foreground">
                {preview.monthlyMinor !== null && preview.monthlyDeductibleMinor !== null && (
                  <>
                    {amount(preview.monthlyMinor)} {t('modals.assetDepreciation.summaryMonthly')}, {amount(preview.monthlyDeductibleMinor)}{' '}
                    {t('modals.assetDepreciation.summaryDeductible')}
                  </>
                )}
              </dd>
              <dt className="text-muted-foreground">{t('modals.assetDepreciation.summaryReclaimed')}</dt>
              <dd className="text-right">{amount(preview.reclaimedVatMinor)}</dd>
              <dd className="text-muted-foreground">{t('modals.assetDepreciation.summaryReclaimedHint')}</dd>
              {preview.lostVatEntryMinor > 0 && (
                <>
                  <dt className="text-muted-foreground">{t('modals.assetDepreciation.summaryLostVat')}</dt>
                  <dd className="text-right">{amount(preview.lostVatEntryMinor)}</dd>
                  <dd className="text-muted-foreground">{t('modals.assetDepreciation.summaryLostVatHint')}</dd>
                </>
              )}
            </dl>
          )}
        </>
      )}
    </>
  )
}

export default AssetPrivateUseFields
