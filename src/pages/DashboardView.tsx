import { useTranslation } from 'react-i18next';
import type { SnapshotRow, EventWithData, Currency, ModalState } from '../types';
import { cn } from '@/lib/utils';
import NumberValue from '../components/NumberValue';
import AccountCards from '../components/AccountCards';
import Ledger from '../components/Ledger';
import { Button } from '../components/ui/button';

interface Props {
  snapshot: SnapshotRow[];
  accounts: SnapshotRow[];
  buckets: SnapshotRow[];
  events: EventWithData[];
  consolidationCurrency: Currency | null;
  totalMinor: number;
  leftToSpendMinor: number;
  missingFxCurrencies: string[];
  setModalState: (state: ModalState) => void;
  isDemoMode: boolean;
  onEnterDemoMode: () => void;
}

export default function DashboardView({
  snapshot,
  accounts,
  buckets,
  events,
  consolidationCurrency,
  totalMinor,
  leftToSpendMinor,
  missingFxCurrencies,
  setModalState,
  isDemoMode,
  onEnterDemoMode,
}: Props) {
  const { t } = useTranslation();

  return (
    <>
      {/* FX rate missing warning */}
      {missingFxCurrencies.length > 0 && (
        <div className="mx-4 md:mx-10 mb-4 rounded-md border border-yellow-400 bg-yellow-50 px-4 py-2 text-sm text-yellow-800 dark:border-yellow-500/40 dark:bg-yellow-900/20 dark:text-yellow-300">
          {t('metrics.fxRateMissing', { currencies: missingFxCurrencies.join(', ') })}
        </div>
      )}

      {/* Hero metrics */}
      <div
        className={cn(
          'px-4 md:px-10 py-10',
          buckets.length > 0 ? 'grid grid-cols-2' : 'flex justify-center',
        )}
      >
        <div className="flex flex-col items-center">
          <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">
            {t('metrics.totalBalance')}
          </p>
          <p className={cn('text-5xl font-extrabold', totalMinor < 0 && 'text-destructive')}>
            <NumberValue
              value={totalMinor}
              currencyCode={consolidationCurrency?.code}
              minorUnits={consolidationCurrency?.minorUnits ?? 2}
            />
          </p>
        </div>
        {buckets.length > 0 && (
          <div className="flex flex-col items-center">
            <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">
              {t('metrics.leftToSpend')}
            </p>
            <p
              className={cn('text-5xl font-extrabold', leftToSpendMinor < 0 && 'text-destructive')}
            >
              <NumberValue
                value={leftToSpendMinor}
                currencyCode={consolidationCurrency?.code}
                minorUnits={consolidationCurrency?.minorUnits ?? 2}
              />
            </p>
          </div>
        )}
      </div>

      <hr className="border-border" />

      {/* Accounts */}
      <div className="px-4 md:px-10 py-8">
        {accounts.length === 0 && !isDemoMode ? (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <h3 className="text-xl font-semibold mb-2">{t('demo.emptyTitle')}</h3>
            <p className="text-sm text-muted-foreground mb-6 max-w-xs">{t('demo.emptyDesc')}</p>
            <div className="flex gap-3">
              <Button onClick={onEnterDemoMode}>{t('demo.emptyCta')}</Button>
              <Button
                variant="outline"
                onClick={() => setModalState({ type: 'createAccount', accountType: 'account' })}
              >
                {t('accounts.addAccount')}
              </Button>
            </div>
          </div>
        ) : (
          <AccountCards
            snapshot={accounts}
            consolidationCurrency={consolidationCurrency}
            sectionTitle={t('accounts.sectionTitle')}
            addButtonLabel={t('accounts.addAccount')}
            onUpdateBalance={(accountId) =>
              setModalState({ type: 'createBalanceUpdate', preselectedAccountId: accountId })
            }
            onRenameAccount={(accountId, currentName) =>
              setModalState({ type: 'renameAccount', accountId, currentName })
            }
            onDeleteAccount={(accountId, name) =>
              setModalState({
                type: 'confirmDeleteAccount',
                accountId,
                name,
                accountType: 'account',
              })
            }
            onCreateAccount={() => setModalState({ type: 'createAccount', accountType: 'account' })}
            onReorder={() => setModalState({ type: 'reorderAccounts' })}
          />
        )}
      </div>

      {accounts.length > 0 && (
        <>
          <hr className="border-border" />

          {/* Buckets */}
          <div className="px-4 md:px-10 py-8">
            <AccountCards
              snapshot={buckets}
              consolidationCurrency={consolidationCurrency}
              sectionTitle={t('buckets.sectionTitle')}
              addButtonLabel={t('buckets.addBucket')}
              emptyMessage={t('buckets.empty')}
              onUpdateBalance={(accountId) =>
                setModalState({ type: 'createBalanceUpdate', preselectedAccountId: accountId })
              }
              onRenameAccount={(accountId, currentName) =>
                setModalState({ type: 'renameAccount', accountId, currentName })
              }
              onDeleteAccount={(accountId, name) =>
                setModalState({
                  type: 'confirmDeleteAccount',
                  accountId,
                  name,
                  accountType: 'bucket',
                })
              }
              onCreateAccount={() =>
                setModalState({ type: 'createAccount', accountType: 'bucket' })
              }
              onReorder={() => setModalState({ type: 'reorderBuckets' })}
            />
          </div>

          {/* Ledger */}
          <div className="px-4 md:px-10 py-8 border-t border-border">
            <Ledger
              events={events}
              accounts={snapshot}
              consolidationCurrency={consolidationCurrency}
              onEditEvent={(event) => setModalState({ type: 'editBalanceUpdate', event })}
              onDeleteEvent={(eventId) => setModalState({ type: 'confirmDeleteEvent', eventId })}
              onUpdateBalances={() => setModalState({ type: 'bulkUpdateBalance' })}
            />
          </div>
        </>
      )}
    </>
  );
}
