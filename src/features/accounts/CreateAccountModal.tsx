import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { PINNED_CURRENCY_CODES } from '../../shared/config/constants';
import { toast } from 'sonner';
import { toMinorUnits, getMinorUnitsStep } from '../../shared/utils/format';
import type { Currency, SnapshotRow } from '../../shared/types';
import { listCurrencies, getConsolidationCurrency } from '../../shared/api';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from '../../shared/ui/dialog';
import { Button } from '../../shared/ui/button';
import { CurrencyInput } from '../../shared/ui/CurrencyInput';
import { Input } from '../../shared/ui/input';
import { Label } from '../../shared/ui/label';
import CurrencySelect from '../currency/CurrencySelect';

interface Props {
  accountType?: 'account' | 'bucket' | 'asset';
  assets?: SnapshotRow[];
  onSubmit: (
    name: string,
    currencyId: number,
    initialBalanceMinor?: number,
    accountType?: string,
    linkedAssetIds?: number[],
  ) => void;
  onClose: () => void;
}

export default function CreateAccountModal({ accountType, assets, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [balance, setBalance] = useState('');
  const [linkedAssetIds, setLinkedAssetIds] = useState<number[]>([]);
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null);

  const isBucket = accountType === 'bucket';
  const isAsset = accountType === 'asset';

  useEffect(() => {
    // Exclude custom units from currency list for accounts and buckets (not assets)
    const includeCustom = accountType === 'asset' ? undefined : false;
    Promise.all([listCurrencies(includeCustom), getConsolidationCurrency()])
      .then(([all, consolidation]) => {
        setCurrencies(all);
        setSelectedCurrency(consolidation);
      })
      .catch((err) => console.error('Failed to load currencies:', err));
  }, [accountType]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.error(
        t('validation.nameRequired', {
          entity: t(isAsset ? 'common.asset' : isBucket ? 'common.bucket' : 'common.account'),
        }),
      );
      return;
    }
    if (!selectedCurrency) {
      toast.error(t('validation.currencyRequired'));
      return;
    }
    let initialBalanceMinor: number | undefined;
    if (balance.trim()) {
      const parsed = parseFloat(balance);
      if (isNaN(parsed)) {
        toast.error(t('validation.invalidBalance'));
        return;
      }
      initialBalanceMinor = toMinorUnits(balance, selectedCurrency.minorUnits);
    }
    onSubmit(
      name.trim(),
      selectedCurrency.id,
      initialBalanceMinor,
      accountType,
      linkedAssetIds.length > 0 ? linkedAssetIds : undefined,
    );
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
          <DialogTitle>
            {t(
              isAsset
                ? 'modals.createAsset.title'
                : isBucket
                  ? 'modals.createBucket.title'
                  : 'modals.createAccount.title',
            )}
          </DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-account-name">
                {t(
                  isAsset
                    ? 'modals.createAsset.nameLabel'
                    : isBucket
                      ? 'modals.createBucket.nameLabel'
                      : 'modals.createAccount.nameLabel',
                )}
              </Label>
              <Input
                id="create-account-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t(
                  isAsset
                    ? 'modals.createAsset.namePlaceholder'
                    : isBucket
                      ? 'modals.createBucket.namePlaceholder'
                      : 'modals.createAccount.namePlaceholder',
                )}
                required
                autoFocus
              />
            </div>

            <div className="flex flex-col gap-2">
              <Label>{t('currency.label')}</Label>
              <CurrencySelect
                currencies={currencies}
                value={selectedCurrency}
                onChange={setSelectedCurrency}
                pinnedCurrencyCodes={PINNED_CURRENCY_CODES}
              />
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="create-account-balance">
                {t(
                  isAsset
                    ? 'modals.createAsset.initialValue'
                    : 'modals.createAccount.initialBalance',
                )}
              </Label>
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

            {!isBucket && !isAsset && assets && assets.length > 0 && (
              <div className="flex flex-col gap-2">
                <Label>{t('modals.createAccount.securedByAsset')}</Label>
                <div className="flex flex-col gap-1">
                  {assets.map((asset) => (
                    <label
                      key={asset.accountId}
                      className="flex items-center gap-2 text-sm cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={linkedAssetIds.includes(asset.accountId)}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setLinkedAssetIds((prev) => [...prev, asset.accountId]);
                          } else {
                            setLinkedAssetIds((prev) =>
                              prev.filter((id) => id !== asset.accountId),
                            );
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
  );
}
