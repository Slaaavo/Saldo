import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown } from 'lucide-react';
import type { Currency } from '../types';
import { listCurrencies, getConsolidationCurrency } from '../api';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';

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
  const [currencySearch, setCurrencySearch] = useState('');
  const [showDropdown, setShowDropdown] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const PINNED_CURRENCIES = ['EUR', 'USD', 'BTC'];
  const isBucket = accountType === 'bucket';

  useEffect(() => {
    Promise.all([listCurrencies(), getConsolidationCurrency()])
      .then(([all, consolidation]) => {
        setCurrencies(all);
        setSelectedCurrency(consolidation);
      })
      .catch((err) => console.error('Failed to load currencies:', err));
  }, []);

  const isSearching = currencySearch.trim() !== '';

  const filteredCurrencies = currencies.filter(
    (c) =>
      c.code.toLowerCase().includes(currencySearch.toLowerCase()) ||
      c.name.toLowerCase().includes(currencySearch.toLowerCase()),
  );

  const pinnedCurrencies = !isSearching
    ? PINNED_CURRENCIES.map((code) => filteredCurrencies.find((c) => c.code === code)).filter(
        (c): c is Currency => c !== undefined,
      )
    : [];

  const unpinnedCurrencies = !isSearching
    ? filteredCurrencies
        .filter((c) => !PINNED_CURRENCIES.includes(c.code))
        .sort((a, b) => a.code.localeCompare(b.code))
    : filteredCurrencies;

  const handleCurrencySelect = (currency: Currency) => {
    setSelectedCurrency(currency);
    setCurrencySearch('');
    setShowDropdown(false);
  };

  const handleCurrencyInputFocus = () => {
    setCurrencySearch('');
    setShowDropdown(true);
  };

  const handleCurrencyInputBlur = () => {
    // Delay close to allow click on dropdown item to register
    setTimeout(() => setShowDropdown(false), 150);
  };

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
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
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
                className={showDropdown ? 'pr-10' : 'cursor-pointer pr-10'}
                readOnly={!showDropdown}
              />
              <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
              {showDropdown && (
                <div className="absolute z-50 mt-1 w-full max-h-48 overflow-y-auto rounded-md border border-border bg-popover shadow-md">
                  {filteredCurrencies.length === 0 ? (
                    <div className="px-3 py-2 text-sm text-muted-foreground">
                      {t('currency.noResults')}
                    </div>
                  ) : isSearching ? (
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
                  ) : (
                    <>
                      {pinnedCurrencies.map((c) => (
                        <button
                          key={c.id}
                          type="button"
                          className="w-full px-3 py-2 text-left text-sm hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground outline-none"
                          onMouseDown={() => handleCurrencySelect(c)}
                        >
                          <span className="font-medium">{c.code}</span>
                          <span className="ml-2 text-muted-foreground">{c.name}</span>
                        </button>
                      ))}
                      {pinnedCurrencies.length > 0 && <hr className="border-border" />}
                      {unpinnedCurrencies.map((c) => (
                        <button
                          key={c.id}
                          type="button"
                          className="w-full px-3 py-2 text-left text-sm hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground outline-none"
                          onMouseDown={() => handleCurrencySelect(c)}
                        >
                          <span className="font-medium">{c.code}</span>
                          <span className="ml-2 text-muted-foreground">{c.name}</span>
                        </button>
                      ))}
                    </>
                  )}
                </div>
              )}
            </div>
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
