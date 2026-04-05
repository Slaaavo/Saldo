import { useTranslation } from 'react-i18next'
import type { SnapshotRow } from '../../shared/types'
import { useImportWizard } from './csv-import/useImportWizard'
import UploadStep from './csv-import/UploadStep'
import MappingStep from './csv-import/MappingStep'
import ReviewStep from './csv-import/ReviewStep'
import RulesStep from './csv-import/RulesStep'
import SaveProfileStep from './csv-import/SaveProfileStep'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody } from '../../shared/ui/dialog'

interface CsvImportModalProps {
  snapshot: SnapshotRow[]
  onClose: () => void
  onSuccess: () => Promise<void>
}

const CsvImportModal = ({ snapshot, onClose, onSuccess }: CsvImportModalProps) => {
  const { t } = useTranslation()

  const {
    wizardState,
    availableBuckets,
    availableProfiles,
    selectedCount,
    duplicateCount,
    nearDateSkippedCount,
    balanceWarningDates,
    accountsWithoutIban,
    selectedAccountCurrencyCode,
    selectedAccountMinorUnits,
    splitValidationErrors,
    handleFileSelect,
    handleAccountSelect,
    canProceedToMapping,
    goToMapping,
    handleMappingChange,
    canProceedToReview,
    goToRules,
    goToReview,
    setRules,
    handleProfileSelect,
    handleToggleRow,
    handleSelectAll,
    handleDeselectAll,
    handleBucketChange,
    handleCounterpartChange,
    handleCreatePartner,
    handleAssignIban,
    handleSplitOpen,
    handleSplitCancel,
    handleLegAmountChange,
    handleLegNoteChange,
    handleLegPartnerChange,
    handleLegBucketChange,
    handleAddLeg,
    handleRemoveLeg,
    handleImport,
    handleSaveNewProfile,
    handleUpdateProfile,
    handleSkipSaveProfile,
    goBack,
  } = useImportWizard({ snapshot, onClose, onSuccess })

  const accountOnlyRows = snapshot.filter((r) => r.accountType === 'account')
  const allAccounts = snapshot.filter((r) => r.accountType === 'account' || r.accountType === 'partner')

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent className="max-w-4xl">
        <DialogHeader>
          <DialogTitle>{t('import.title')}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          {wizardState.step === 'upload' && (
            <UploadStep
              fileName={wizardState.file?.name ?? null}
              selectedAccountId={wizardState.selectedAccountId}
              accounts={accountOnlyRows}
              profiles={availableProfiles}
              selectedProfileId={wizardState.loadedProfileId}
              onProfileSelect={handleProfileSelect}
              onFileSelect={handleFileSelect}
              onAccountSelect={handleAccountSelect}
              onNext={goToMapping}
              onCancel={onClose}
              canProceed={canProceedToMapping}
            />
          )}

          {wizardState.step === 'mapping' && (
            <MappingStep
              csvHeaders={wizardState.csvHeaders}
              columnMapping={wizardState.columnMapping}
              onMappingChange={handleMappingChange}
              onNext={goToRules}
              onBack={goBack}
              onCancel={onClose}
              canProceed={canProceedToReview}
            />
          )}

          {wizardState.step === 'rules' && (
            <RulesStep rules={wizardState.rules} csvHeaders={wizardState.csvHeaders} onRulesChange={setRules} onNext={goToReview} onBack={goBack} onCancel={onClose} />
          )}

          {wizardState.step === 'review' && (
            <ReviewStep
              importRows={wizardState.importRows}
              selectedCount={selectedCount}
              duplicateCount={duplicateCount}
              nearDateSkippedCount={nearDateSkippedCount}
              balanceWarningDates={balanceWarningDates}
              availableBuckets={availableBuckets}
              accountsWithoutIban={accountsWithoutIban}
              allAccounts={allAccounts}
              selectedAccountCurrencyCode={selectedAccountCurrencyCode}
              selectedAccountMinorUnits={selectedAccountMinorUnits}
              importing={wizardState.importing}
              onToggleRow={handleToggleRow}
              onSelectAll={handleSelectAll}
              onDeselectAll={handleDeselectAll}
              onBucketChange={handleBucketChange}
              onCreatePartner={handleCreatePartner}
              onAssignIban={handleAssignIban}
              onCounterpartChange={handleCounterpartChange}
              onImport={handleImport}
              onBack={goBack}
              onCancel={onClose}
              onSplitOpen={handleSplitOpen}
              onSplitCancel={handleSplitCancel}
              onLegAmountChange={handleLegAmountChange}
              onLegNoteChange={handleLegNoteChange}
              onLegPartnerChange={handleLegPartnerChange}
              onLegBucketChange={handleLegBucketChange}
              onAddLeg={handleAddLeg}
              onRemoveLeg={handleRemoveLeg}
              splitValidationErrors={splitValidationErrors}
            />
          )}

          {wizardState.step === 'save-profile' && (
            <SaveProfileStep
              importedCount={wizardState.importedCount ?? 0}
              loadedProfileName={wizardState.loadedProfileId !== null ? (availableProfiles.find((p) => p.id === wizardState.loadedProfileId)?.name ?? null) : null}
              onSaveNew={handleSaveNewProfile}
              onUpdate={handleUpdateProfile}
              onClose={handleSkipSaveProfile}
            />
          )}
        </DialogBody>
      </DialogContent>
    </Dialog>
  )
}

export default CsvImportModal
