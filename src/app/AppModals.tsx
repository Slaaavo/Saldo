import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { fetchFxRates } from '../shared/api';
import type { ModalState, SnapshotRow, Currency } from '../shared/types';
import { useModalActions } from './useModalActions';
import CreateBalanceUpdateModal from '../features/transactions/CreateBalanceUpdateModal';
import EditBalanceUpdateModal from '../features/transactions/EditBalanceUpdateModal';
import CreateAccountModal from '../features/accounts/CreateAccountModal';
import CreateAssetModal from '../features/assets/CreateAssetModal';
import UpdateAssetValueModal from '../features/assets/UpdateAssetValueModal';
import RenameAccountModal from '../features/accounts/RenameAccountModal';
import ConfirmDialog from '../shared/ui/ConfirmDialog';
import BulkUpdateBalanceModal from '../features/transactions/BulkUpdateBalanceModal';
import DbLocationChoiceDialog from '../features/settings/DbLocationChoiceDialog';
import ReorderModal from '../shared/ui/ReorderModal';
import ManageLinkedAssetsModal from '../features/assets/ManageLinkedAssetsModal';

interface AppModalsProps {
  modalState: ModalState;
  closeModal: () => void;
  setModalState: (state: ModalState) => void;
  snapshot: SnapshotRow[];
  accounts: SnapshotRow[];
  buckets: SnapshotRow[];
  assets: SnapshotRow[];
  selectedDate: string;
  consolidationCurrency: Currency | null;
  refresh: () => Promise<void>;
  dbLocation: {
    actionLoading: boolean;
    handleConfirmSwitch: (folder: string) => Promise<void>;
    handleLocationChoiceAction: (action: string, folder: string, isReset: boolean) => Promise<void>;
    handleConfirmReset: () => Promise<void>;
  };
}

export default function AppModals({
  modalState,
  closeModal,
  setModalState,
  snapshot,
  accounts,
  buckets,
  assets,
  selectedDate,
  consolidationCurrency,
  refresh,
  dbLocation,
}: AppModalsProps) {
  const { t } = useTranslation();
  const [fetchLoading, setFetchLoading] = useState(false);

  const {
    handleCreateBalanceUpdate,
    handleEditBalanceUpdate,
    handleDeleteEvent,
    handleCreateAccount,
    handleRenameAccount,
    handleDeleteAccount,
    handleBulkUpdateSubmit,
    handleSaveOrder,
    handleUpdateAssetValue,
    handleSetAccountAssetLinks,
    handleCreateAssetSuccess,
  } = useModalActions({
    closeModal,
    refresh,
    snapshot,
    consolidationCurrency,
    onFxRatePrompt: (date) => setModalState({ type: 'fetchFxRatePrompt', date }),
  });

  switch (modalState.type) {
    case 'createBalanceUpdate':
      return (
        <CreateBalanceUpdateModal
          accounts={snapshot}
          preselectedAccountId={modalState.preselectedAccountId}
          onSubmit={handleCreateBalanceUpdate}
          onClose={closeModal}
        />
      );

    case 'editBalanceUpdate':
      return (
        <EditBalanceUpdateModal
          event={modalState.event}
          accounts={snapshot}
          onSubmit={handleEditBalanceUpdate}
          onClose={closeModal}
        />
      );

    case 'createAccount':
      return (
        <CreateAccountModal
          accountType={modalState.accountType}
          assets={assets}
          onSubmit={handleCreateAccount}
          onClose={closeModal}
        />
      );

    case 'createAsset':
      return <CreateAssetModal onSuccess={handleCreateAssetSuccess} onClose={closeModal} />;

    case 'renameAccount':
      return (
        <RenameAccountModal
          accountId={modalState.accountId}
          currentName={modalState.currentName}
          onSubmit={handleRenameAccount}
          onClose={closeModal}
        />
      );

    case 'confirmDeleteAccount':
      return (
        <ConfirmDialog
          message={
            modalState.accountType === 'asset' &&
            accounts.some((a) => a.linkedAssetIds.includes(modalState.accountId))
              ? t('modals.confirm.deleteAssetWithLinks', {
                  entityType: t('common.asset'),
                  name: modalState.name,
                  accounts: accounts
                    .filter((a) => a.linkedAssetIds.includes(modalState.accountId))
                    .map((a) => a.accountName)
                    .join(', '),
                })
              : t('modals.confirm.deleteAccount', {
                  entityType: t(
                    modalState.accountType === 'bucket'
                      ? 'common.bucket'
                      : modalState.accountType === 'asset'
                        ? 'common.asset'
                        : 'common.account',
                  ),
                  name: modalState.name,
                })
          }
          onConfirm={() => handleDeleteAccount(modalState.accountId)}
          onCancel={closeModal}
        />
      );

    case 'confirmDeleteEvent':
      return (
        <ConfirmDialog
          message={t('modals.confirm.deleteEvent')}
          onConfirm={() => handleDeleteEvent(modalState.eventId)}
          onCancel={closeModal}
        />
      );

    case 'bulkUpdateBalance':
      return (
        <BulkUpdateBalanceModal
          accounts={snapshot}
          selectedDate={selectedDate}
          onSubmit={handleBulkUpdateSubmit}
          onClose={closeModal}
        />
      );

    case 'fetchFxRatePrompt':
      return (
        <ConfirmDialog
          message={t('modals.fetchFxRatePrompt.message', { date: modalState.date })}
          confirmVariant="default"
          loading={fetchLoading}
          onConfirm={async () => {
            setFetchLoading(true);
            try {
              await fetchFxRates(modalState.date);
              await refresh();
            } catch {
              // Silently ignore fetch errors — user can retry from FX Rates screen
            } finally {
              setFetchLoading(false);
              closeModal();
            }
          }}
          onCancel={closeModal}
        />
      );

    case 'updateAssetValue':
      return (
        <UpdateAssetValueModal
          accountId={modalState.accountId}
          accountName={modalState.accountName}
          currencyCode={modalState.currencyCode}
          currencyMinorUnits={modalState.currencyMinorUnits}
          balanceMinor={modalState.balanceMinor}
          consolidationCurrency={consolidationCurrency}
          onSubmit={handleUpdateAssetValue}
          onClose={closeModal}
        />
      );

    case 'reorderAccounts':
      return (
        <ReorderModal
          items={accounts.map((r) => ({ id: r.accountId, name: r.accountName }))}
          title={t('reorder.titleAccounts')}
          onSave={handleSaveOrder}
          onClose={closeModal}
        />
      );

    case 'reorderBuckets':
      return (
        <ReorderModal
          items={buckets.map((r) => ({ id: r.accountId, name: r.accountName }))}
          title={t('reorder.titleBuckets')}
          onSave={handleSaveOrder}
          onClose={closeModal}
        />
      );

    case 'reorderAssets':
      return (
        <ReorderModal
          items={assets.map((r) => ({ id: r.accountId, name: r.accountName }))}
          title={t('reorder.titleAssets')}
          onSave={handleSaveOrder}
          onClose={closeModal}
        />
      );

    case 'manageLinkedAssets':
      return (
        <ManageLinkedAssetsModal
          accountId={modalState.accountId}
          accountName={modalState.accountName}
          assets={assets}
          currentLinks={
            snapshot.find((r) => r.accountId === modalState.accountId)?.linkedAssetIds ?? []
          }
          onSave={handleSetAccountAssetLinks}
          onClose={closeModal}
        />
      );

    case 'confirmSwitchDb':
      return (
        <ConfirmDialog
          message={t('dataStorage.confirmSwitchMessage')}
          confirmVariant="default"
          loading={dbLocation.actionLoading}
          onConfirm={() => dbLocation.handleConfirmSwitch(modalState.folder)}
          onCancel={closeModal}
        />
      );

    case 'dbLocationChoice':
      return (
        <DbLocationChoiceDialog
          loading={dbLocation.actionLoading}
          onAction={(action) =>
            dbLocation.handleLocationChoiceAction(action, modalState.folder, modalState.isReset)
          }
          onCancel={closeModal}
        />
      );

    case 'confirmResetDbLocation':
      return (
        <ConfirmDialog
          message={t('dataStorage.confirmResetMessage')}
          confirmVariant="default"
          loading={dbLocation.actionLoading}
          onConfirm={dbLocation.handleConfirmReset}
          onCancel={closeModal}
        />
      );

    default:
      return null;
  }
}
