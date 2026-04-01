import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'

interface Props {
  accountId: number
  currentName: string
  accountType: string
  currentIban?: string | null
  onSubmit: (accountId: number, name: string, iban?: string) => void
  onClose: () => void
}

export default function EditAccountModal({ accountId, currentName, accountType, currentIban, onSubmit, onClose }: Props) {
  const { t } = useTranslation()
  const [name, setName] = useState(currentName)
  const [iban, setIban] = useState(currentIban ?? '')

  const showIban = accountType === 'account'

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('common.account') }))
      return
    }
    if (showIban) {
      onSubmit(accountId, name.trim(), iban.trim())
    } else {
      onSubmit(accountId, name.trim())
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
          <DialogTitle>{t('modals.editAccount.title')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-account-name">{t('modals.editAccount.nameLabel')}</Label>
              <Input id="edit-account-name" type="text" value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
            </div>
            {showIban && (
              <div className="flex flex-col gap-2">
                <Label htmlFor="edit-account-iban">{t('modals.editAccount.ibanLabel')}</Label>
                <Input id="edit-account-iban" type="text" value={iban} onChange={(e) => setIban(e.target.value)} placeholder={t('modals.editAccount.ibanPlaceholder')} />
              </div>
            )}
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {t('modals.editAccount.cancel')}
            </Button>
            <Button type="submit">{t('modals.editAccount.submit')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
