import { useTranslation } from 'react-i18next';
import type { EventWithData, SnapshotRow, Currency } from '../types';
import { formatDisplayDate } from '../utils/format';
import { cn } from '@/lib/utils';
import NumberValue from './NumberValue';
import BucketAmountWithTooltip from './BucketAmountWithTooltip';
import { Button } from './ui/button';
import { Card, CardContent } from './ui/card';
import { Pencil, RefreshCw, Trash2 } from 'lucide-react';

interface Props {
  events: EventWithData[];
  accounts: SnapshotRow[];
  consolidationCurrency?: Currency | null;
  onEditEvent: (event: EventWithData) => void;
  onDeleteEvent: (eventId: number) => void;
  onUpdateBalances: () => void;
}

export default function Ledger({
  events,
  accounts,
  consolidationCurrency,
  onEditEvent,
  onDeleteEvent,
  onUpdateBalances,
}: Props) {
  const { t } = useTranslation();

  // Build account position map from snapshot order
  const accountPosition = new Map<number, number>();
  accounts.forEach((a, i) => accountPosition.set(a.accountId, i));

  // Build lookup map from accountId to SnapshotRow
  const accountMap = new Map<number, SnapshotRow>();
  accounts.forEach((a) => accountMap.set(a.accountId, a));

  // Group events by date
  const groupMap = new Map<string, EventWithData[]>();
  for (const ev of events) {
    const dateKey = ev.eventDate.substring(0, 10);
    if (!groupMap.has(dateKey)) {
      groupMap.set(dateKey, []);
    }
    groupMap.get(dateKey)!.push(ev);
  }

  // Sort date groups: most recent first
  const sortedGroups = [...groupMap.entries()].sort(([a], [b]) => b.localeCompare(a));

  // Sort events within each group by account position
  for (const [, groupEvents] of sortedGroups) {
    groupEvents.sort(
      (a, b) => (accountPosition.get(a.accountId) ?? 0) - (accountPosition.get(b.accountId) ?? 0),
    );
  }

  // Identify the most recent event per bucket (for showing current balance tooltip)
  const latestBucketEventIds = new Set<number>();
  const seenBucketAccountIds = new Set<number>();
  for (const [, groupEvents] of sortedGroups) {
    for (const ev of groupEvents) {
      if (ev.accountType === 'bucket' && !seenBucketAccountIds.has(ev.accountId)) {
        latestBucketEventIds.add(ev.id);
        seenBucketAccountIds.add(ev.accountId);
      }
    }
  }

  return (
    <section>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold">{t('ledger.title')}</h2>
        <Button onClick={onUpdateBalances} size="sm">
          <RefreshCw className="h-4 w-4" />
          {t('ledger.updateBalances')}
        </Button>
      </div>

      {events.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('ledger.empty')}</p>
      ) : (
        <div className="flex flex-col gap-6">
          {sortedGroups.map(([dateKey, groupEvents]) => (
            <div key={dateKey}>
              <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">
                {formatDisplayDate(dateKey)}
              </h3>
              <div className="flex flex-col gap-2">
                {groupEvents.map((ev) => {
                  const bucketSnap = latestBucketEventIds.has(ev.id)
                    ? accountMap.get(ev.accountId)
                    : undefined;
                  return (
                    <Card key={ev.id}>
                      <CardContent className="flex items-center justify-between p-4">
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-semibold truncate">{ev.accountName}</p>
                          {ev.note && (
                            <p className="text-xs text-muted-foreground italic truncate">
                              {ev.note}
                            </p>
                          )}
                        </div>
                        <div className="flex items-center gap-6">
                          {bucketSnap ? (
                            <BucketAmountWithTooltip
                              totalMinor={bucketSnap.convertedBalanceMinor}
                              manualBalanceMinor={bucketSnap.balanceMinor}
                              allocations={bucketSnap.linkedAllocations}
                              currencyCode={consolidationCurrency?.code ?? bucketSnap.currencyCode}
                              minorUnits={
                                consolidationCurrency?.minorUnits ?? bucketSnap.currencyMinorUnits
                              }
                              manualCurrencyCode={bucketSnap.currencyCode}
                              manualMinorUnits={bucketSnap.currencyMinorUnits}
                              className={cn(
                                'text-sm font-bold tabular-nums',
                                bucketSnap.convertedBalanceMinor < 0 && 'text-destructive',
                              )}
                            />
                          ) : (
                            <NumberValue
                              value={ev.amountMinor}
                              currencyCode={ev.currencyCode}
                              minorUnits={ev.currencyMinorUnits}
                              className={cn(
                                'text-sm font-bold tabular-nums',
                                ev.amountMinor < 0 && 'text-destructive',
                              )}
                            />
                          )}
                          <div className="flex items-center gap-1">
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8"
                              onClick={() => onEditEvent(ev)}
                            >
                              <Pencil className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8"
                              onClick={() => onDeleteEvent(ev.id)}
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
