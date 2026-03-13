import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../../shared/types';
import { Popover, PopoverContent, PopoverTrigger } from '../../shared/ui/popover';
import { Button } from '../../shared/ui/button';
import { Check, ChevronDown } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface Props {
  accounts: SnapshotRow[];
  selectedIds: number[];
  onChange: (ids: number[]) => void;
}

interface Group {
  labelKey: string;
  type: string;
  items: SnapshotRow[];
}

export default function PortfolioItemFilter({ accounts, selectedIds, onChange }: Props) {
  const { t } = useTranslation();

  const groups: Group[] = [
    {
      labelKey: 'ledgerPage.groupAccounts',
      type: 'account',
      items: accounts.filter((a) => a.accountType === 'account'),
    },
    {
      labelKey: 'ledgerPage.groupBuckets',
      type: 'bucket',
      items: accounts.filter((a) => a.accountType === 'bucket'),
    },
    {
      labelKey: 'ledgerPage.groupAssets',
      type: 'asset',
      items: accounts.filter((a) => a.accountType === 'asset'),
    },
  ].filter((g) => g.items.length > 0);

  const selectedSet = new Set(selectedIds);

  const toggle = (id: number) => {
    if (selectedSet.has(id)) {
      onChange(selectedIds.filter((i) => i !== id));
    } else {
      onChange([...selectedIds, id]);
    }
  };

  const label =
    selectedIds.length > 0
      ? t('ledgerPage.portfolioItemCount', { count: selectedIds.length })
      : t('ledgerPage.portfolioItem');

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" className="justify-between gap-2">
          <span>{label}</span>
          <ChevronDown className="size-4 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-64 p-2" align="start">
        <div className="max-h-72 overflow-y-auto flex flex-col gap-3">
          {groups.map((group) => (
            <div key={group.type}>
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide px-2 mb-1">
                {t(group.labelKey)}
              </p>
              {group.items.map((item) => {
                const selected = selectedSet.has(item.accountId);
                return (
                  <button
                    key={item.accountId}
                    onClick={() => toggle(item.accountId)}
                    className={cn(
                      'w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors text-left',
                      selected
                        ? 'bg-accent text-accent-foreground'
                        : 'hover:bg-muted text-foreground',
                    )}
                  >
                    <Check
                      className={cn('size-4 shrink-0', selected ? 'opacity-100' : 'opacity-0')}
                    />
                    <span className="truncate">{item.accountName}</span>
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
