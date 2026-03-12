import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown } from 'lucide-react';
import type { Currency } from '../../shared/types';
import { cn } from '@/shared/lib/utils';
import { Input } from '../../shared/ui/input';

function CurrencyOption({
  currency,
  onSelect,
}: {
  currency: Currency;
  onSelect: (c: Currency) => void;
}) {
  return (
    <button
      type="button"
      className="w-full px-3 py-2 text-left text-sm hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground outline-none"
      onMouseDown={() => onSelect(currency)}
    >
      <span className="font-medium inline-block w-10">{currency.code}</span>
      <span className="text-muted-foreground">{currency.name}</span>
    </button>
  );
}

interface CurrencySelectProps {
  currencies: Currency[];
  value: Currency | null;
  onChange: (currency: Currency) => void;
  pinnedCurrencyCodes?: string[];
  className?: string;
}

export default function CurrencySelect({
  currencies,
  value,
  onChange,
  pinnedCurrencyCodes,
  className,
}: CurrencySelectProps) {
  const { t } = useTranslation();
  const [currencySearch, setCurrencySearch] = useState('');
  const [showDropdown, setShowDropdown] = useState(false);

  const isSearching = currencySearch.trim() !== '';

  const filteredCurrencies = currencies.filter(
    (c) =>
      c.code.toLowerCase().includes(currencySearch.toLowerCase()) ||
      c.name.toLowerCase().includes(currencySearch.toLowerCase()),
  );

  const pinnedCurrencies =
    pinnedCurrencyCodes && !isSearching
      ? pinnedCurrencyCodes
          .map((code) => filteredCurrencies.find((c) => c.code === code))
          .filter((c): c is Currency => c !== undefined)
      : [];

  const unpinnedCurrencies =
    pinnedCurrencyCodes && !isSearching
      ? filteredCurrencies
          .filter((c) => !pinnedCurrencyCodes.includes(c.code))
          .sort((a, b) => a.code.localeCompare(b.code))
      : filteredCurrencies;

  const handleFocus = () => {
    setCurrencySearch('');
    setShowDropdown(true);
  };

  const handleBlur = () => {
    setTimeout(() => setShowDropdown(false), 150);
  };

  return (
    <div className={cn('relative', className)}>
      <Input
        type="text"
        value={showDropdown ? currencySearch : value ? `${value.code} — ${value.name}` : ''}
        onChange={(e) => setCurrencySearch(e.target.value)}
        onFocus={handleFocus}
        onBlur={handleBlur}
        placeholder={t('currency.searchPlaceholder')}
        readOnly={!showDropdown}
        className={cn('pr-10', !showDropdown && 'cursor-pointer')}
      />
      <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none h-4 w-4 opacity-50" />
      {showDropdown && (
        <div className="absolute z-50 mt-1 w-full max-h-48 overflow-y-auto rounded-md border border-border bg-popover shadow-md">
          {filteredCurrencies.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">{t('currency.noResults')}</div>
          ) : isSearching || !pinnedCurrencyCodes ? (
            filteredCurrencies.map((c) => (
              <CurrencyOption key={c.id} currency={c} onSelect={onChange} />
            ))
          ) : (
            <>
              {pinnedCurrencies.map((c) => (
                <CurrencyOption key={c.id} currency={c} onSelect={onChange} />
              ))}
              {pinnedCurrencies.length > 0 && <hr className="border-border" />}
              {unpinnedCurrencies.map((c) => (
                <CurrencyOption key={c.id} currency={c} onSelect={onChange} />
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
