import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { Currency } from '../types';
import { listCurrencies, getConsolidationCurrency } from '../api';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import CurrencySelect from './CurrencySelect';

interface Props {
  accountType?: 'account' | 'bucket';
  onSubmit: (
    name: string,
    currencyId: number,
    initialBalanceMinor?: number,
    accountType?: string,
  ) => void;
  onClose: () => void;
}

export default function CreateAccountModal({ accountType, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [balance, setBalance] = useState('');
  const [currencies, setCurrencies] = useState<Currency[]>([]);
  const [selectedCurrency, setSelectedCurrency] = useState<Currency | null>(null);

  const isBucket = accountType === 'bucket';

  useEffect(() => {
    Promise.all([listCurrencies(), getConsolidationCurrency()])
      .then(([all, consolidation]) => {
        setCurrencies(all);
        setSelectedCurrency(consolidation);
      })
      .catch((err) => console.error('Failed to load currencies:', err));
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      window.alert(
        t('validation.nameRequired', { entity: t(isBucket ? 'common.bucket' : 'common.account') }),
      );
      return;
    }
    if (!selectedCurrency) {
      window.alert(t('validation.currencyRequired'));
      return;
    }
    let initialBalanceMinor: number | undefined;
    if (balance.trim()) {
      const parsed = parseFloat(balance);
      if (isNaN(parsed)) {
        window.alert(t('validation.invalidBalance'));
        return;
      }
      initialBalanceMinor = Math.round(parsed * Math.pow(10, selectedCurrency.minorUnits));
    }
    onSubmit(name.trim(), selectedCurrency.id, initialBalanceMinor, accountType);
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
            {t(isBucket ? 'modals.createBucket.title' : 'modals.createAccount.title')}
          </DialogTitle>
        </DialogHeader>
        <form
          onSubmit={handleSubmit}
          className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4"
        >
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-account-name">
                {t(isBucket ? 'modals.createBucket.nameLabel' : 'modals.createAccount.nameLabel')}
              </Label>
              <Input
                id="create-account-name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t(
                  isBucket
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
                pinnedCurrencyCodes={['EUR', 'USD', 'BTC']}
              />
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="create-account-balance">
                {t('modals.createAccount.initialBalance')}
              </Label>
              <div className="relative">
                <Input
                  id="create-account-balance"
                  type="number"
                  step={
                    !selectedCurrency || selectedCurrency.minorUnits === 0
                      ? '1'
                      : '0.' + '0'.repeat(selectedCurrency.minorUnits - 1) + '1'
                  }
                  value={balance}
                  onChange={(e) => setBalance(e.target.value)}
                  placeholder="0"
                  className={selectedCurrency ? 'pr-14' : undefined}
                />
                {selectedCurrency && (
                  <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground pointer-events-none">
                    {selectedCurrency.code}
                  </span>
                )}
              </div>
            </div>
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
