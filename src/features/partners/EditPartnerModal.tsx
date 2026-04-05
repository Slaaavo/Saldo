import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { PartnerAccount } from '../../shared/types'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { extractErrorMessage } from '../../shared/utils/errors'

interface Props {
  partner: PartnerAccount
  onSubmit: (accountId: number, name: string, iban: string) => Promise<void>
  onClose: () => void
}

const EditPartnerModal = ({ partner, onSubmit, onClose }: Props) => {
  const { t } = useTranslation()
  const [name, setName] = useState(partner.name)
  const [iban, setIban] = useState(partner.iban ?? '')
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.account') }))
      return
    }
    if (!iban.trim()) {
      toast.error(t('partners.errors.invalidIban'))
      return
    }
    setSubmitting(true)
    try {
      await onSubmit(partner.id, name.trim(), iban.trim())
    } catch (err) {
      const msg = extractErrorMessage(err)
      if (msg.includes('DUPLICATE_IBAN') || msg.toLowerCase().includes('already in use')) {
        toast.error(t('partners.errors.duplicateIban'))
      } else if (msg.includes('15') || msg.toLowerCase().includes('alphanumeric')) {
        toast.error(t('partners.errors.invalidIban'))
      } else {
        toast.error(t('errors.updatePartner', { error: msg }))
      }
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
          <DialogTitle>{t('modals.editPartner.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-partner-name">{t('modals.editPartner.nameLabel')}</Label>
              <Input id="edit-partner-name" type="text" value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-partner-iban">{t('modals.editPartner.ibanLabel')}</Label>
              <Input id="edit-partner-iban" type="text" value={iban} onChange={(e) => setIban(e.target.value)} required />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editPartner.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('modals.editPartner.submit')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditPartnerModal
