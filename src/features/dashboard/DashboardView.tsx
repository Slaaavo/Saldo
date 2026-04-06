import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EventWithData, Currency } from '../../shared/types'
import { getEventById, listPersons } from '../../shared/api'
import { extractErrorMessage } from '../../shared/utils/errors'
import { cn } from '@/shared/lib/utils'
import NumberValue from '../../shared/ui/NumberValue'
import AccountCards from './AccountCards'
import Ledger from './Ledger'
import { Button } from '../../shared/ui/button'
import { useDemo } from '../../app/useDemo'
import { useModal } from '../../app/useModal'
import { useSelectedDate } from '../../app/useSelectedDate'
import { useSnapshotQuery } from '../../shared/hooks/useSnapshotQuery'
import { useDashboardEventsQuery } from '../../shared/hooks/useDashboardEventsQuery'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { computeDashboardMetrics } from './dashboardMetrics'
import { useNavigate } from '@tanstack/react-router'

const MetricCard = ({ label, value, currency }: { label: string; value: number; currency: Currency | null }) => {
  return (
    <div className="flex flex-col items-center">
      <p className="text-sm font-medium uppercase tracking-widest text-muted-foreground mb-1">{label}</p>
      <p className={cn('text-4xl font-extrabold', value < 0 && 'text-destructive')}>
        <NumberValue value={value} currencyCode={currency?.code} minorUnits={currency?.minorUnits ?? 2} />
      </p>
    </div>
  )
}

const sectionClass = 'px-4 md:px-10 py-8'

const metricsGridClass = (count: number) => {
  if (count === 3) return 'grid grid-cols-3'
  if (count === 2) return 'grid grid-cols-2'
  return 'flex justify-center'
}

const DashboardView = () => {
  const { t } = useTranslation()
  const { isDemoMode, onEnterDemoMode } = useDemo()
  const { setModalState } = useModal()
  const { selectedDate } = useSelectedDate()
  const navigate = useNavigate()

  const snapshotQuery = useSnapshotQuery(selectedDate)
  const eventsQuery = useDashboardEventsQuery(selectedDate)
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const personsQuery = useQuery({ queryKey: ['persons'], queryFn: listPersons })

  const snapshot = snapshotQuery.data ?? []
  const { accounts, buckets, assets, hasAssets, liquidMinor, leftToSpendMinor, netWorthMinor } = computeDashboardMetrics(snapshot)
  const events = eventsQuery.data?.events ?? []
  const totalEvents = eventsQuery.data?.totalCount ?? 0
  const consolidationCurrency = consolidationCurrencyQuery.data ?? null
  const persons = personsQuery.data ?? []
  const missingFxCurrencies = [...new Set(snapshot.filter((r) => r.fxRateMissing).map((r) => r.currencyCode))]

  if (snapshotQuery.isPending && eventsQuery.isPending && consolidationCurrencyQuery.isPending) {
    return null
  }

  const metrics = [
    ...(hasAssets
      ? [
          { key: 'netWorth', label: t('metrics.netWorth'), value: netWorthMinor },
          { key: 'liquid', label: t('metrics.liquid'), value: liquidMinor },
        ]
      : [{ key: 'totalBalance', label: t('metrics.totalBalance'), value: liquidMinor }]),
    ...(buckets.length > 0 ? [{ key: 'leftToSpend', label: t('metrics.leftToSpend'), value: leftToSpendMinor }] : []),
  ]

  const handleRename = (accountId: number, currentName: string) => {
    const row = snapshot.find((r) => r.accountId === accountId)
    setModalState({
      type: 'editAccount',
      accountId,
      currentName,
      accountType: row?.accountType ?? 'account',
      currentIban: row?.iban,
      currentPersonId: row?.personId,
    })
  }

  const handleDelete = (accountType: 'account' | 'bucket' | 'asset') => (accountId: number, name: string) =>
    setModalState({ type: 'confirmDeleteAccount', accountId, name, accountType })

  const handleUpdateBalance = (accountId: number) => setModalState({ type: 'createBalanceUpdate', preselectedAccountId: accountId })

  const handleUpdateAssetValue = (accountId: number) => {
    const row = assets.find((a) => a.accountId === accountId)
    if (row?.isCustom) {
      setModalState({
        type: 'updateAssetValue',
        accountId,
        accountName: row.accountName,
        currencyCode: row.currencyCode,
        currencyMinorUnits: row.currencyMinorUnits,
        isCustomUnit: true,
        balanceMinor: row.balanceMinor,
      })
    } else {
      handleUpdateBalance(accountId)
    }
  }

  const handleEditEvent = async (event: EventWithData) => {
    if (event.eventType === 'balance_update') {
      setModalState({ type: 'editBalanceUpdate', event })
      return
    }
    if (event.eventType === 'transfer') {
      if (!event.linkedEventId) {
        toast.error(t('errors.loadData'))
        return
      }
      try {
        const linked = await getEventById(event.linkedEventId)
        if (!linked) {
          toast.error(t('errors.loadData'))
          return
        }
        const fromEvent = event.amountMinor < 0 ? event : linked
        const toEvent = event.amountMinor < 0 ? linked : event
        setModalState({ type: 'editTransfer', fromEvent, toEvent })
      } catch (err) {
        toast.error(extractErrorMessage(err))
      }
    }
  }

  return (
    <>
      {/* FX rate missing warning */}
      {missingFxCurrencies.length > 0 && (
        <div className="mx-4 md:mx-10 mb-4 rounded-md border border-yellow-400 bg-yellow-50 px-4 py-2 text-sm text-yellow-800 dark:border-yellow-500/40 dark:bg-yellow-900/20 dark:text-yellow-300">
          {t('metrics.fxRateMissing', { currencies: missingFxCurrencies.join(', ') })}
        </div>
      )}

      {/* Hero metrics */}
      <div className={cn('px-4 md:px-10 py-10', metricsGridClass(metrics.length))}>
        {metrics.map(({ key, label, value }) => (
          <MetricCard key={key} label={label} value={value} currency={consolidationCurrency} />
        ))}
      </div>

      <hr className="border-border" />

      {/* Accounts */}
      <div className={sectionClass}>
        {accounts.length === 0 && !isDemoMode ? (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <h3 className="text-xl font-semibold mb-2">{t('demo.emptyTitle')}</h3>
            <p className="text-sm text-muted-foreground mb-6 max-w-xs">{t('demo.emptyDesc')}</p>
            <div className="flex gap-3">
              <Button onClick={onEnterDemoMode}>{t('demo.emptyCta')}</Button>
              <Button variant="outline" onClick={() => setModalState({ type: 'createAccount', accountType: 'account' })}>
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
            onUpdateBalance={handleUpdateBalance}
            onRenameAccount={handleRename}
            onDeleteAccount={handleDelete('account')}
            onCreateAccount={() => setModalState({ type: 'createAccount', accountType: 'account' })}
            onReorder={() => setModalState({ type: 'reorderAccounts' })}
            onManageLinkedAssets={(accountId, accountName) => setModalState({ type: 'manageLinkedAssets', accountId, accountName })}
            allAssets={assets}
            persons={persons}
          />
        )}
      </div>

      {accounts.length > 0 && (
        <>
          <hr className="border-border" />

          {/* Buckets */}
          <div className={sectionClass}>
            <AccountCards
              snapshot={buckets}
              consolidationCurrency={consolidationCurrency}
              sectionTitle={t('buckets.sectionTitle')}
              addButtonLabel={t('buckets.addBucket')}
              emptyMessage={t('buckets.empty')}
              onUpdateBalance={handleUpdateBalance}
              onRenameAccount={handleRename}
              onDeleteAccount={handleDelete('bucket')}
              onCreateAccount={() => setModalState({ type: 'createAccount', accountType: 'bucket' })}
              onReorder={() => setModalState({ type: 'reorderBuckets' })}
              persons={persons}
            />
          </div>

          <hr className="border-border" />

          {/* Assets */}
          <div className={sectionClass}>
            <AccountCards
              snapshot={assets}
              consolidationCurrency={consolidationCurrency}
              sectionTitle={t('assets.sectionTitle')}
              addButtonLabel={t('assets.addAsset')}
              emptyMessage={t('assets.empty')}
              updateButtonLabel={t('assets.updateValue')}
              onUpdateBalance={handleUpdateAssetValue}
              onRenameAccount={handleRename}
              onDeleteAccount={handleDelete('asset')}
              onCreateAccount={() => setModalState({ type: 'createAsset' })}
              onReorder={() => setModalState({ type: 'reorderAssets' })}
              allAccounts={accounts}
              persons={persons}
            />
          </div>

          {/* Ledger */}
          <div className="px-4 md:px-10 py-8 border-t border-border">
            <Ledger
              events={events}
              accounts={snapshot}
              consolidationCurrency={consolidationCurrency}
              onEditEvent={handleEditEvent}
              onDeleteEvent={(eventId) => setModalState({ type: 'confirmDeleteEvent', eventId })}
              onDeleteTransferEvent={(eventId, linkedEventId) => setModalState({ type: 'confirmDeleteTransferEvent', eventId, linkedEventId })}
              onUpdateBalances={() => setModalState({ type: 'bulkUpdateBalance' })}
              onImportCsv={() => setModalState({ type: 'csvImport' })}
              totalEvents={totalEvents}
              onViewAll={() => navigate({ to: '/ledger' })}
            />
          </div>
        </>
      )}
    </>
  )
}

export default DashboardView
