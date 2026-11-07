import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { getAssetUse, listPersons } from '../../shared/api'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import type { AssetUseTerms } from '../../shared/types'
import { fromMinorUnits, toMinorUnits } from '../../shared/utils/format'
import AssetPrivateUseFields from '../assets/AssetPrivateUseFields'
import BusinessUseChanges from '../assets/BusinessUseChanges'
import { DEPRECIATION_PRESETS, EMPTY_ASSET_USE_DRAFT, assetUseProblem, draftFromTerms, termsFromDraft, type AssetUseDraft } from '../assets/assetUseDraft'
import { useAssetPurchasePreview } from '../assets/useAssetPurchasePreview'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { DatePicker } from '../../shared/ui/date-picker'
import { Label } from '../../shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'

interface Props {
  accountId: number
  currentName: string
  accountType: string
  currentIban?: string | null
  currentPersonId?: number | null
  isCustomUnit: boolean
  currencyCode: string
  currencyMinorUnits: number
  currentPurchasePriceMinor: number | null
  currentPurchaseDate: string | null
  currentDepreciationPeriodMonths: number | null
  onSubmit: (
    accountId: number,
    name: string,
    iban?: string,
    personId?: number,
    purchasePriceMinor?: number | null,
    purchaseDate?: string | null,
    depreciationPeriodMonths?: number | null,
    assetUse?: AssetUseTerms | null,
  ) => void
  onClose: () => void
}

const EditAccountModal = ({
  accountId,
  currentName,
  accountType,
  currentIban,
  currentPersonId,
  isCustomUnit,
  currencyCode,
  currencyMinorUnits,
  currentPurchasePriceMinor,
  currentPurchaseDate,
  currentDepreciationPeriodMonths,
  onSubmit,
  onClose,
}: Props) => {
  const { t } = useTranslation()
  const { data: persons } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const [name, setName] = useState(currentName)
  const [iban, setIban] = useState(currentIban ?? '')
  const [personId, setPersonId] = useState<number | null>(currentPersonId ?? null)

  const showIban = accountType === 'account'
  const showPersonSelector = accountType !== 'partner' && !!persons && persons.length > 1
  const selectedPerson = persons?.find((p) => p.id === personId)
  const { data: consolidationCurrency } = useConsolidationCurrencyQuery()
  // Depreciation is booked in the consolidation currency. An asset that already has depreciation data keeps the
  // fields visible so the user can clear them instead of losing them unseen.
  const showDepreciation =
    accountType === 'asset' && !isCustomUnit && selectedPerson?.personType === 'legal' && (currencyCode === consolidationCurrency?.code || currentPurchasePriceMinor !== null)

  // Depreciation state — currency of asset is needed for CurrencyInput but we don't have it here;
  // we use a generic "amount" input with no currency symbol (the parent supplies currencyCode via snapshot)
  const [purchasePrice, setPurchasePrice] = useState(currentPurchasePriceMinor !== null ? fromMinorUnits(currentPurchasePriceMinor, currencyMinorUnits) : '')
  const [purchaseDate, setPurchaseDate] = useState(currentPurchaseDate ? currentPurchaseDate.slice(0, 10) : '')
  const [depreciationPeriod, setDepreciationPeriod] = useState(currentDepreciationPeriodMonths !== null ? String(currentDepreciationPeriodMonths) : '')

  const { data: storedUse } = useQuery({ queryKey: ['assetUse', accountId], queryFn: () => getAssetUse(accountId), enabled: accountType === 'asset' })
  const [editedUse, setEditedUse] = useState<AssetUseDraft | null>(null)
  const assetUse = editedUse ?? (storedUse ? draftFromTerms(storedUse, currencyMinorUnits) : EMPTY_ASSET_USE_DRAFT)
  const showVatFields = selectedPerson?.vatPayer === true || !!storedUse?.purchaseVat
  const preview = useAssetPurchasePreview({ draft: assetUse, minorUnits: currencyMinorUnits, showVatFields: showVatFields && showDepreciation, depreciationPeriod })
  const hasPeriod = parseInt(depreciationPeriod, 10) > 0

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.account') }))
      return
    }
    const useProblem = showDepreciation && storedUse ? assetUseProblem(assetUse, currencyMinorUnits, showVatFields) : null
    if (useProblem) {
      toast.error(t(`modals.assetDepreciation.${useProblem}`))
      return
    }
    const purchasePriceMinor = showDepreciation && purchasePrice.trim() ? toMinorUnits(purchasePrice, currencyMinorUnits) : null
    const purchaseDateValue = showDepreciation && purchaseDate.trim() ? purchaseDate.trim() : null
    const periodMonths = showDepreciation && depreciationPeriod.trim() !== '' ? Math.max(0, parseInt(depreciationPeriod, 10)) : null
    const assetUseTerms = showDepreciation && storedUse ? termsFromDraft(assetUse, currencyMinorUnits, showVatFields) : null
    if (showIban) {
      onSubmit(accountId, name.trim(), iban.trim(), personId ?? undefined, purchasePriceMinor, purchaseDateValue, periodMonths, assetUseTerms)
    } else {
      onSubmit(accountId, name.trim(), undefined, personId ?? undefined, purchasePriceMinor, purchaseDateValue, periodMonths, assetUseTerms)
    }
  }

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.editAccount.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-account-name">{t('modals.editAccount.nameLabel')}</Label>
              <Input id="edit-account-name" type="text" value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
            </div>
            {showIban && (
              <div className="flex flex-col gap-2">
                <Label htmlFor="edit-account-iban">{t('modals.editAccount.ibanLabel')}</Label>
                <Input id="edit-account-iban" type="text" value={iban} onChange={(e) => setIban(e.target.value)} placeholder={t('modals.editAccount.ibanPlaceholder')} />
              </div>
            )}
            {showPersonSelector && (
              <div className="flex flex-col gap-2">
                <Label>{t('persons.selector')}</Label>
                <Select value={personId === null ? 'none' : String(personId)} onValueChange={(val) => setPersonId(val === 'none' ? null : Number(val))}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {persons!.map((p) => (
                      <SelectItem key={p.id} value={String(p.id)}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
            {showDepreciation && (
              <>
                <h3 className="text-base font-semibold border-t border-border pt-2 mt-1">{t('modals.assetDepreciation.sectionTitle')}</h3>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-purchase-price">{t('modals.assetDepreciation.purchasePrice')}</Label>
                  <CurrencyInput
                    id="edit-purchase-price"
                    value={preview ? fromMinorUnits(preview.depreciatingMinor, currencyMinorUnits) : purchasePrice}
                    onChange={(e) => setPurchasePrice(e.target.value)}
                    disabled={preview !== undefined}
                    currencyCode=""
                  />
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-purchase-date">{t('modals.assetDepreciation.purchaseDate')}</Label>
                  <DatePicker id="edit-purchase-date" value={purchaseDate} onChange={setPurchaseDate} />
                </div>
                <div className="flex flex-col gap-2">
                  <Label htmlFor="edit-depreciation-period">{t('modals.assetDepreciation.depreciationPeriodMonths')}</Label>
                  <Input id="edit-depreciation-period" type="number" min="0" value={depreciationPeriod} onChange={(e) => setDepreciationPeriod(e.target.value)} placeholder="" />
                  <p className="text-xs text-muted-foreground">{t('modals.assetDepreciation.depreciationPeriodHelper')}</p>
                  <div className="flex gap-2 flex-wrap">
                    {DEPRECIATION_PRESETS.map(([key, val]) => (
                      <Button key={key} type="button" variant="outline" size="sm" onClick={() => setDepreciationPeriod(val)}>
                        {t(`modals.assetDepreciation.${key}`)}
                      </Button>
                    ))}
                  </div>
                </div>
                <AssetPrivateUseFields
                  idPrefix="edit-asset"
                  draft={assetUse}
                  onChange={setEditedUse}
                  showVatFields={showVatFields}
                  currencyCode={currencyCode}
                  minorUnits={currencyMinorUnits}
                  preview={preview}
                />
                {(hasPeriod || assetUse.changes.length > 0) && (
                  <BusinessUseChanges
                    idPrefix="edit-asset"
                    atPurchasePct={assetUse.privateUse ? assetUse.businessUsePct : '100'}
                    changes={assetUse.changes}
                    onChange={(changes) => setEditedUse({ ...assetUse, changes })}
                  />
                )}
              </>
            )}
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editAccount.cancel')}
            </Button>
            <Button type="submit">{t('modals.editAccount.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditAccountModal
