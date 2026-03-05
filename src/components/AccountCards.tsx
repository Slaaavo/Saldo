import { useRef, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { SnapshotRow } from '../types';
import NumberValue from './NumberValue';
import { cn } from '@/lib/utils';
import { Button } from './ui/button';
import { Card, CardContent } from './ui/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';
import { MoreVertical, Plus, Pencil, Trash2, ChevronLeft, ChevronRight } from 'lucide-react';

interface Props {
  snapshot: SnapshotRow[];
  sectionTitle?: string;
  addButtonLabel?: string;
  emptyMessage?: string;
  onUpdateBalance: (accountId: number) => void;
  onRenameAccount: (accountId: number, currentName: string) => void;
  onDeleteAccount: (accountId: number, name: string) => void;
  onCreateAccount: () => void;
}

export default function AccountCards({
  snapshot,
  sectionTitle,
  addButtonLabel,
  emptyMessage,
  onUpdateBalance,
  onRenameAccount,
  onDeleteAccount,
  onCreateAccount,
}: Props) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const updateScrollButtons = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 0);
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  }, []);

  useEffect(() => {
    updateScrollButtons();
    const el = scrollRef.current;
    if (!el) return;
    el.addEventListener('scroll', updateScrollButtons, { passive: true });
    const ro = new ResizeObserver(updateScrollButtons);
    ro.observe(el);
    return () => {
      el.removeEventListener('scroll', updateScrollButtons);
      ro.disconnect();
    };
  }, [updateScrollButtons, snapshot]);

  const scroll = useCallback((direction: 'left' | 'right') => {
    const el = scrollRef.current;
    if (!el) return;
    const amount = direction === 'left' ? -220 : 220;
    el.scrollBy({ left: amount, behavior: 'smooth' });
  }, []);

  return (
    <section>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold">{sectionTitle ?? t('accounts.sectionTitle')}</h2>
        <Button onClick={onCreateAccount} size="sm" variant="outline">
          <Plus className="h-4 w-4" />
          {addButtonLabel ?? t('accounts.addAccount')}
        </Button>
      </div>

      {snapshot.length === 0 ? (
        <p className="text-sm text-muted-foreground">{emptyMessage ?? t('accounts.empty')}</p>
      ) : (
        <div className="group/scroll relative">
          {canScrollLeft && (
            <button
              onClick={() => scroll('left')}
              className="absolute left-0 top-1/2 -translate-y-1/2 z-10 hidden group-hover/scroll:flex h-8 w-8 items-center justify-center rounded-full bg-background border border-border shadow-md hover:bg-accent transition-colors"
              aria-label={t('accounts.scrollLeft')}
            >
              <ChevronLeft className="h-4 w-4" />
            </button>
          )}

          <div
            ref={scrollRef}
            className="flex gap-4 overflow-x-auto no-scrollbar"
            style={{ scrollbarWidth: 'none' }}
          >
            {snapshot.map((row) => (
              <Card key={row.accountId} className="relative w-[200px] min-w-[200px] shrink-0">
                <CardContent className="flex flex-col gap-1 p-4">
                  <div className="flex items-start justify-between min-w-0">
                    <span
                      className="text-sm text-muted-foreground truncate"
                      title={row.accountName}
                    >
                      {row.accountName}
                    </span>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-7 w-7 -mr-2 -mt-1">
                          <MoreVertical className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          onClick={() => onRenameAccount(row.accountId, row.accountName)}
                        >
                          <Pencil className="h-4 w-4" />
                          {t('accounts.rename')}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          className="text-destructive focus:text-destructive"
                          onClick={() => onDeleteAccount(row.accountId, row.accountName)}
                        >
                          <Trash2 className="h-4 w-4" />
                          {t('accounts.delete')}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                  <NumberValue
                    value={row.balanceMinor}
                    className={cn('text-2xl font-bold', row.balanceMinor < 0 && 'text-destructive')}
                  />
                  <button
                    onClick={() => onUpdateBalance(row.accountId)}
                    className="mt-2 text-xs font-bold uppercase tracking-wider text-muted-foreground hover:text-primary transition-colors flex items-center gap-1"
                  >
                    <Pencil className="h-3 w-3" />
                    {t('accounts.updateBalance')}
                  </button>
                </CardContent>
              </Card>
            ))}
          </div>

          {canScrollRight && (
            <button
              onClick={() => scroll('right')}
              className="absolute right-0 top-1/2 -translate-y-1/2 z-10 hidden group-hover/scroll:flex h-8 w-8 items-center justify-center rounded-full bg-background border border-border shadow-md hover:bg-accent transition-colors"
              aria-label={t('accounts.scrollRight')}
            >
              <ChevronRight className="h-4 w-4" />
            </button>
          )}
        </div>
      )}
    </section>
  );
}
