import { useEffect, useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useQueryClient } from '@tanstack/react-query'
import { useResolvedPersonId } from '../transactions/useResolvedPersonId'
import { useEkasaImportWizard } from './useEkasaImportWizard'
import UploadStep from './UploadStep'
import RulesStep from './RulesStep'
import ReviewStep from './ReviewStep'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { extractErrorMessage } from '../../shared/utils/errors'

interface Props {
  onClose: () => void
}

const STEP_TITLE_KEYS = {
  upload: 'ekasaImport.title.upload',
  rules: 'ekasaImport.title.rules',
  review: 'ekasaImport.title.review',
  'save-profile': 'ekasaImport.title.saveProfile',
  error: 'ekasaImport.title.error',
} as const

const EkasaImportModal = ({ onClose }: Props) => {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { personId } = useResolvedPersonId()
  const {
    state,
    handleFileSelect,
    handleRulesConfirm: wizardHandleRulesConfirm,
    handleBack,
    handleTryAgain,
    handleOfflineFallback,
    handleImportComplete,
    loadProfile,
    saveProfile,
    profileChanged,
    setRules,
    setDefaultDeductiblePct,
    setDefaultVatReclaimablePct,
  } = useEkasaImportWizard()

  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (personId !== null) {
      void loadProfile(personId)
    }
  }, [personId, loadProfile])

  const handleRulesConfirm = useCallback(async () => {
    await wizardHandleRulesConfirm()
  }, [wizardHandleRulesConfirm])

  const handleImportSuccess = useCallback(() => {
    if (profileChanged()) {
      handleImportComplete()
    } else {
      onClose()
    }
  }, [profileChanged, handleImportComplete, onClose])

  const handleSaveProfile = useCallback(async () => {
    if (personId === null) return
    setSaving(true)
    try {
      await saveProfile(personId)
      await queryClient.invalidateQueries({ queryKey: ['ekasaProfiles'] })
      toast.success(t('ekasaImport.saveStep.profileSaved'))
      onClose()
    } catch (err) {
      toast.error(extractErrorMessage(err))
    } finally {
      setSaving(false)
    }
  }, [personId, saveProfile, queryClient, t, onClose])

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle>{t(STEP_TITLE_KEYS[state.step])}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          {state.step === 'upload' && <UploadStep onFileSelect={handleFileSelect} />}

          {state.step === 'rules' && (
            <RulesStep
              rules={state.rules}
              defaultDeductiblePct={state.defaultDeductiblePct}
              defaultVatReclaimablePct={state.defaultVatReclaimablePct}
              isProcessing={state.isProcessing}
              onRulesChange={setRules}
              onDefaultDeductiblePctChange={setDefaultDeductiblePct}
              onDefaultVatReclaimablePctChange={setDefaultVatReclaimablePct}
              onConfirm={handleRulesConfirm}
              onBack={handleBack}
            />
          )}

          {state.step === 'review' && (
            <ReviewStep
              processedLegs={state.processedLegs}
              vendorName={state.vendorName}
              receiptDate={state.receiptDate}
              onImportSuccess={handleImportSuccess}
              onBack={handleBack}
            />
          )}

          {state.step === 'save-profile' && (
            <div className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">{t('ekasaImport.saveStep.prompt')}</p>
              <div className="flex justify-end gap-2">
                <Button type="button" variant="outline" onClick={onClose} disabled={saving}>
                  {t('ekasaImport.saveStep.skip')}
                </Button>
                <Button type="button" onClick={() => void handleSaveProfile()} disabled={saving}>
                  {t('ekasaImport.saveStep.save')}
                </Button>
              </div>
            </div>
          )}

          {state.step === 'error' && (
            <div className="flex flex-col gap-4">
              <p className="text-sm text-destructive">{state.receiptError}</p>
              <div className="flex gap-2">
                <Button type="button" variant="outline" onClick={handleTryAgain}>
                  {t('ekasaImport.errorStep.tryAgain')}
                </Button>
                {state.offlineFallback !== null && (
                  <Button type="button" onClick={handleOfflineFallback}>
                    {t('ekasaImport.errorStep.continuePartial')}
                  </Button>
                )}
              </div>
            </div>
          )}
        </DialogBody>
      </DialogContent>
    </Dialog>
  )
}

export default EkasaImportModal
