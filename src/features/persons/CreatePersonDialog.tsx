import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter } from '../../shared/ui/dialog'
import { Button } from '../../shared/ui/button'
import { Input } from '../../shared/ui/input'
import { Label } from '../../shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'
import { Checkbox } from '../../shared/ui/checkbox'
import { extractErrorMessage } from '../../shared/utils/errors'

interface Props {
  open: boolean
  onOpenChange: (v: boolean) => void
  onSubmit: (name: string, personType: string, vatPayer: boolean) => Promise<void>
}

const CreatePersonDialog = ({ open, onOpenChange, onSubmit }: Props) => {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [personType, setPersonType] = useState('physical')
  const [vatPayer, setVatPayer] = useState(false)
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('persons.name') }))
      return
    }
    setSubmitting(true)
    try {
      await onSubmit(name.trim(), personType, vatPayer)
      setName('')
      setPersonType('physical')
      setVatPayer(false)
      setSubmitting(false)
      onOpenChange(false)
    } catch (err) {
      toast.error(extractErrorMessage(err))
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('persons.createPerson')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-person-name">{t('persons.name')}</Label>
              <Input id="create-person-name" type="text" value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="create-person-type">{t('persons.type')}</Label>
              <Select value={personType} onValueChange={setPersonType}>
                <SelectTrigger id="create-person-type">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="physical">{t('persons.typePhysical')}</SelectItem>
                  <SelectItem value="legal">{t('persons.typeLegal')}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-center gap-2">
              <Checkbox id="create-person-vat-payer" checked={vatPayer} onCheckedChange={(v) => setVatPayer(v === true)} />
              <Label htmlFor="create-person-vat-payer">{t('persons.vatPayer')}</Label>
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('modals.createPartner.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('persons.createPerson')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default CreatePersonDialog
