import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import type { EventWithData, MetricKind } from '../../shared/types'
import { listPersons } from '../../shared/api'
import { cn } from '@/shared/lib/utils'
import AccountCards from './AccountCards'
import Ledger from './Ledger'
import MetricCard from './MetricCard'
import MetricBreakdownDialog from './MetricBreakdownDialog'
import RateWarnings from './RateWarnings'
import TimeMachineBanner from './TimeMachineBanner'
import { Button } from '../../shared/ui/button'
import { useDemo } from '../../app/useDemo'
import { useModal } from '../../app/useModal'
import { useTimeMachine } from '../../app/useTimeMachine'
import { useSnapshotQuery } from '../../shared/hooks/useSnapshotQuery'
import { useDashboardEventsQuery } from '../../shared/hooks/useDashboardEventsQuery'
import { useConsolidationCurrencyQuery } from '../../shared/hooks/useConsolidationCurrencyQuery'
import { useEditEventHandler } from '../../shared/hooks/useEditEventHandler'
import { EMPTY_SNAPSHOT, groupRowsByKind } from '../../shared/utils/snapshot'
import { useNavigate } from '@tanstack/react-router'

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
  const { asOf, backToLive } = useTimeMachine()
  const navigate = useNavigate()
  const handleEditEvent = useEditEventHandler()
  const [openMetric, setOpenMetric] = useState<MetricKind | null>(null)

  const snapshotQuery = useSnapshotQuery(asOf)
  const eventsQuery = useDashboardEventsQuery(asOf)
  const consolidationCurrencyQuery = useConsolidationCurrencyQuery()
  const personsQuery = useQuery({ queryKey: ['persons'], queryFn: listPersons })

  const { rows: snapshot, metrics, rateWarnings } = snapshotQuery.data ?? EMPTY_SNAPSHOT
  const { accounts, buckets, assets } = groupRowsByKind(snapshot)
  const events = eventsQuery.data?.events ?? []
  const totalEvents = eventsQuery.data?.totalCount ?? 0
  const consolidationCurrency = consolidationCurrencyQuery.data ?? null
  const persons = personsQuery.data ?? []
  const breakdown = metrics.find((m) => m.kind === openMetric)

  // Everything below, including the empty state, depends on the snapshot, so wait for it rather than for all three queries.
  if (snapshotQuery.isPending) {
    return null
  }

  const handleRename = (accountId: number, currentName: string) => {
    const row = snapshot.find((r) => r.accountId === accountId)
    setModalState({
      type: 'editAccount',
      accountId,
      currentName,
      accountType: row?.accountType ?? 'account',
      currentIban: row?.iban,
      currentPersonId: row?.personId,
      isCustomUnit: row?.isCustom ?? false,
      currencyCode: row?.currencyCode ?? '',
      currencyMinorUnits: row?.currencyMinorUnits ?? 2,
      currentPurchasePriceMinor: row?.purchasePriceMinor ?? null,
      currentPurchaseDate: row?.purchaseDate ?? null,
      currentDepreciationPeriodMonths: row?.depreciationPeriodMonths ?? null,
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

  const handleEditTaxableSplitGroup = (splitGroupId: number, eventType: string, legs: EventWithData[], groupNote: string | null, accountId: number) => {
    setModalState({ type: 'editTaxableSplitGroup', splitGroupId, eventType: eventType as 'revenue' | 'expense', legs, groupNote, accountId })
  }

  return (
    <>
      {asOf.kind === 'at' && <TimeMachineBanner until={asOf.until} onBackToLive={backToLive} />}

      {rateWarnings.length > 0 && <RateWarnings warnings={rateWarnings} />}

      {/* Hero metrics */}
      <div className={cn('px-4 md:px-10 py-10', metricsGridClass(metrics.length))}>
        {metrics.map((metric) => (
          <MetricCard key={metric.kind} metric={metric} currency={consolidationCurrency} onOpen={() => setOpenMetric(metric.kind)} />
        ))}
      </div>

      {breakdown && consolidationCurrency && <MetricBreakdownDialog metric={breakdown} currency={consolidationCurrency} onClose={() => setOpenMetric(null)} />}

      <hr className="border-border" />

      {/* Accounts */}
      <div className={sectionClass}>
        {snapshot.length === 0 && !isDemoMode ? (
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

      {snapshot.length > 0 && (
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
              onDeleteEvent={(eventId, eventType) => setModalState({ type: 'confirmDeleteEvent', eventId, eventType })}
              onDeleteTransferEvent={(eventId, linkedEventId) => setModalState({ type: 'confirmDeleteTransferEvent', eventId, linkedEventId })}
              onDeleteSplitGroup={(splitGroupId) => setModalState({ type: 'confirmDeleteSplitGroup', splitGroupId })}
              onAddEvents={() => setModalState({ type: 'addEvents' })}
              onEditTaxableSplitGroup={handleEditTaxableSplitGroup}
              onImportCsv={() => setModalState({ type: 'csvImport' })}
              onImportEkasa={() => setModalState({ type: 'ekasaImport' })}
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
