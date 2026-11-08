import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { useQuery } from '@tanstack/react-query'
import { PINNED_CURRENCY_CODES } from '../../shared/config/constants'
import { toast } from 'sonner'
import { toMinorUnits, getMinorUnitsStep } from '../../shared/utils/format'
import { BUCKET_KINDS, type BucketKind, type Currency, type SnapshotRow } from '../../shared/types'
import { cn } from '../../shared/lib/utils'
import { listCurrencies, getConsolidationCurrency, listPersons } from '../../shared/api'
import { useSelectedPerson } from '../../app/useSelectedPerson'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'
import CurrencySelect from '../currency/CurrencySelect'

interface Props {
  accountType?: 'account' | 'bucket' | 'asset'
  assets?: SnapshotRow[]
  onSubmit: (name: string, currencyId: number, initialBalanceMinor?: number, accountType?: string, linkedAssetIds?: number[], iban?: string, personId?: number) => void
  onSubmitIncomeBudget: (name: string, currencyId: number, personId: number) => void
  onClose: () => void
}

const CreateAccountModal = ({ accountType, assets, onSubmit, onSubmitIncomeBudget, onClose }: Props) => {
  const { t } = useTranslation()
  const { selectedPersonId } = useSelectedPerson()
  const { data: persons } = useQuery({ queryKey: ['persons'], queryFn: listPersons })
  const [name, setName] = useState('')
  const [balance, setBalance] = useState('')
  const [iban, setIban] = useState('')
  const [linkedAssetIds, setLinkedAssetIds] = useState<number[]>([])
  const [currencies, setCurrencies] = useState<Currency[]>([])
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null)
  const [bucketKind, setBucketKind] = useState<BucketKind>('balance')
  const [personMissing, setPersonMissing] = useState(false)

  const isBucket = accountType === 'bucket'
  const isAsset = accountType === 'asset'
  const isIncomeBudget = isBucket && bucketKind === 'incomeBudget'

  const [pickedPersonId, setPickedPersonId] = useState<number | undefined>(selectedPersonId ?? undefined)
  const derivedPersonId = isIncomeBudget ? null : (persons?.find((p) => p.isDefault)?.id ?? null)
  const personId: number | null = pickedPersonId ?? derivedPersonId
  const showPersonSelect = !isAsset && !!persons && (isIncomeBudget || persons.length > 1)

  useEffect(() => {
    // Exclude custom units from currency list for accounts and buckets (not assets)
    const includeCustom = accountType === 'asset' ? undefined : false
    Promise.all([listCurrencies(includeCustom), getConsolidationCurrency()])
      .then(([all, consolidation]) => {
        setCurrencies(all)
        setSelectedCurrency(consolidation)
      })
      .catch((err) => console.error('Failed to load currencies:', err))
  }, [accountType])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      toast.error(
        t('validation.nameRequired', {
          entity: t(isAsset ? 'common.asset' : isBucket ? 'common.bucket' : 'common.account'),
        }),
      )
      return
    }
    if (!selectedCurrency) {
      toast.error(t('validation.currencyRequired'))
      return
    }
    if (isIncomeBudget) {
      if (personId === null) setPersonMissing(true)
      else onSubmitIncomeBudget(name.trim(), selectedCurrency.id, personId)
      return
    }
    let initialBalanceMinor: number | undefined
    if (balance.trim()) {
      const parsed = parseFloat(balance)
      if (isNaN(parsed)) {
        toast.error(t('validation.invalidBalance'))
        return
      }
      initialBalanceMinor = toMinorUnits(balance, selectedCurrency.minorUnits)
    }
    onSubmit(
      name.trim(),
      selectedCurrency.id,
      initialBalanceMinor,
      accountType,
      linkedAssetIds.length > 0 ? linkedAssetIds : undefined,
      !isBucket && !isAsset ? iban.trim() || undefined : undefined,
      personId ?? undefined,
    )
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
          <DialogTitle>{t(isAsset ? 'modals.createAsset.title' : isBucket ? 'modals.createBucket.title' : 'modals.createAccount.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            {isBucket && (
              <div className="flex flex-col gap-2">
                <Label id="create-bucket-kind">{t('modals.createBucket.kind')}</Label>
                <div role="group" aria-labelledby="create-bucket-kind" className="inline-flex w-fit gap-0.5 rounded-md bg-muted p-[3px]">
                  {BUCKET_KINDS.map((kind) => (
                    <button
                      key={kind}
                      type="button"
                      aria-pressed={bucketKind === kind}
                      onClick={() => {
                        setBucketKind(kind)
                        setPersonMissing(false)
                      }}
                      className={cn(
                        'rounded-[calc(var(--radius)-2px)] px-3 py-1.5 text-sm font-semibold text-muted-foreground transition-colors hover:text-foreground',
                        bucketKind === kind && 'bg-card text-foreground shadow-sm',
                      )}
                    >
                      {t(`modals.createBucket.kinds.${kind}`)}
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="flex flex-col gap-2">
              <Label htmlFor="create-account-name">
                {t(isAsset ? 'modals.createAsset.nameLabel' : isBucket ? 'modals.createBucket.nameLabel' : 'modals.createAccount.nameLabel')}
              </Label>
              <Input
                id="create-account-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t(
                  isAsset
                    ? 'modals.createAsset.namePlaceholder'
                    : isIncomeBudget
                      ? 'modals.createBucket.incomeBudgetNamePlaceholder'
                      : isBucket
                        ? 'modals.createBucket.namePlaceholder'
                        : 'modals.createAccount.namePlaceholder',
                )}
                required
                autoFocus
              />
            </div>

            {!isBucket && !isAsset && (
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-account-iban">{t('modals.createAccount.ibanLabel')}</Label>
                <Input id="create-account-iban" type="text" value={iban} onChange={(e) => setIban(e.target.value)} placeholder={t('modals.createAccount.ibanPlaceholder')} />
              </div>
            )}

            <div className="flex flex-col gap-2">
              <Label>{t('currency.label')}</Label>
              <CurrencySelect currencies={currencies} value={selectedCurrency} onChange={setSelectedCurrency} pinnedCurrencyCodes={PINNED_CURRENCY_CODES} />
            </div>

            {showPersonSelect && (
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-account-person">{t('persons.selector')}</Label>
                <Select
                  value={personId === null ? 'none' : String(personId)}
                  onValueChange={(val) => {
                    // Radix also reports '' here when the value has no matching item, so only a listed person counts.
                    const picked = persons.find((p) => String(p.id) === val)
                    if (!picked) return
                    setPickedPersonId(picked.id)
                    setPersonMissing(false)
                  }}
                >
                  <SelectTrigger
                    id="create-account-person"
                    aria-invalid={personMissing || undefined}
                    aria-describedby={personMissing ? 'create-account-person-missing' : undefined}
                  >
                    <SelectValue>{personId === null ? <span className="text-muted-foreground">{t('modals.createBucket.pickPerson')}</span> : undefined}</SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {persons.map((p) => (
                      <SelectItem key={p.id} value={String(p.id)}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {personMissing && (
                  <p id="create-account-person-missing" role="alert" className="text-xs text-destructive">
                    {t('modals.createBucket.personRequired')}
                  </p>
                )}
              </div>
            )}

            {!isIncomeBudget && (
              <div className="flex flex-col gap-2">
                <Label htmlFor="create-account-balance">{t(isAsset ? 'modals.createAsset.initialValue' : 'modals.createAccount.initialBalance')}</Label>
                <CurrencyInput
                  id="create-account-balance"
                  type="number"
                  step={getMinorUnitsStep(selectedCurrency?.minorUnits ?? 0)}
                  value={balance}
                  onChange={(e) => setBalance(e.target.value)}
                  placeholder="0"
                  currencyCode={selectedCurrency?.code}
                />
              </div>
            )}

            {!isBucket && !isAsset && assets && assets.length > 0 && (
              <div className="flex flex-col gap-2">
                <Label>{t('modals.createAccount.securedByAsset')}</Label>
                <div className="flex flex-col gap-1">
                  {assets.map((asset) => (
                    <label key={asset.accountId} className="flex items-center gap-2 text-sm cursor-pointer">
                      <input
                        type="checkbox"
                        checked={linkedAssetIds.includes(asset.accountId)}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setLinkedAssetIds((prev) => [...prev, asset.accountId])
                          } else {
                            setLinkedAssetIds((prev) => prev.filter((id) => id !== asset.accountId))
                          }
                        }}
                      />
                      {asset.accountName}
                    </label>
                  ))}
                </div>
              </div>
            )}
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.createAccount.cancel')}
            </Button>
            <Button type="submit">{t('modals.createAccount.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default CreateAccountModal
