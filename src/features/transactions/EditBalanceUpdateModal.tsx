import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { EventWithData, SnapshotRow } from '../../shared/types'
import { fromMinorUnits, toMinorUnits, getMinorUnitsStep } from '../../shared/utils/format'
import { extractErrorMessage } from '../../shared/utils/errors'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { CurrencyInput } from '../../shared/ui/CurrencyInput'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { DatePicker } from '../../shared/ui/date-picker'
import { useBucketLinks } from '../buckets/useBucketLinks'
import BucketAllocationEditor from '../buckets/BucketAllocationEditor'

interface Props {
  event: EventWithData
  accounts: SnapshotRow[]
  onSubmit: (eventId: number, amountMinor: number, eventDate: string, note: string) => void
  onBucketSubmit?: (eventId: number, amountMinor: number, eventDate: string, note: string | null, linkedAccountIds: number[]) => Promise<void>
  onClose: () => void
}

const EditBalanceUpdateModal = ({ event, accounts, onSubmit, onBucketSubmit, onClose }: Props) => {
  const { t } = useTranslation()
  const minorUnits = event.currencyMinorUnits
  const isBucket = event.accountType === 'bucket'
  const [amount, setAmount] = useState(fromMinorUnits(event.amountMinor, minorUnits))
  const [date, setDate] = useState(event.eventDate)
  const [note, setNote] = useState(event.note ?? '')
  const [constraintError, setConstraintError] = useState<string | null>(null)

  const allocationSources = accounts.filter((a) => a.accountType === 'account' || a.accountType === 'asset')

  const { loadingLinks, visibleLinks, availableToLink, handleSourceAccountSelect, handleAddLink, handleUnlink, handleRemoveNew, getLinkedAccountIds } = useBucketLinks({
    isBucket,
    eventId: isBucket ? event.id : null,
    allAccounts: allocationSources,
  })
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    const parsed = parseFloat(amount)
    if (isNaN(parsed)) {
      toast.error(t('validation.invalidAmount'))
      return
    }
    setSubmitting(true)
    const amountMinor = toMinorUnits(amount, minorUnits)

    if (isBucket && onBucketSubmit) {
      try {
        await onBucketSubmit(event.id, amountMinor, date, note || null, getLinkedAccountIds())
        onClose()
      } catch (err) {
        const appErr = err as { code?: string; message?: string } | null
        if (appErr?.code === 'LINK_CONFLICT') {
          try {
            const payload = JSON.parse(appErr.message ?? '{}') as {
              sourceAccountName: string
              otherBucketName: string
              conflictDate: string
            }
            setConstraintError(
              t('errors.linkConflict', {
                accountName: payload.sourceAccountName,
                bucketName: payload.otherBucketName,
                date: payload.conflictDate,
              }),
            )
          } catch {
            setConstraintError(String(appErr.message))
          }
        } else {
          toast.error(extractErrorMessage(err))
        }
        setSubmitting(false)
      }
      return
    }

    try {
      onSubmit(event.id, amountMinor, date, note)
    } catch (err) {
      toast.error(extractErrorMessage(err))
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('modals.editBalanceUpdate.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label>{t('modals.editBalanceUpdate.account')}</Label>
              <Input type="text" value={event.accountName} disabled className="bg-muted" />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="ebu-amount">{isBucket ? t('modals.createBalanceUpdate.extraBalance') : t('modals.editBalanceUpdate.amount')}</Label>
              <CurrencyInput
                id="ebu-amount"
                type="number"
                step={getMinorUnitsStep(minorUnits)}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                currencyCode={event.currencyCode}
                required
                autoFocus
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="ebu-date">{t('modals.editBalanceUpdate.date')}</Label>
              <DatePicker
                id="ebu-date"
                value={date}
                onChange={(v) => {
                  setDate(v)
                  setConstraintError(null)
                }}
                withTime
                defaultTime="23:59"
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="ebu-note">{t('modals.editBalanceUpdate.note')}</Label>
              <Input id="ebu-note" type="text" value={note} onChange={(e) => setNote(e.target.value)} placeholder={t('modals.editBalanceUpdate.notePlaceholder')} />
            </div>

            {isBucket && (
              <BucketAllocationEditor
                visibleLinks={visibleLinks}
                availableToLink={availableToLink}
                allAccounts={allocationSources}
                loadingLinks={loadingLinks}
                constraintError={constraintError}
                handleSourceAccountSelect={handleSourceAccountSelect}
                handleAddLink={handleAddLink}
                handleRemoveNew={handleRemoveNew}
                handleUnlink={handleUnlink}
              />
            )}
          </DialogBody>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editBalanceUpdate.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.editBalanceUpdate.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditBalanceUpdateModal
