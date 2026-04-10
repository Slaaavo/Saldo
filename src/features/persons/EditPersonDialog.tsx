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
import type { PersonRow } from '../../shared/types'

interface Props {
  open: boolean
  onOpenChange: (v: boolean) => void
  person: PersonRow
  onSubmit: (personId: number, name: string, personType: string, vatPayer: boolean) => Promise<void>
}

const EditPersonDialog = ({ open, onOpenChange, person, onSubmit }: Props) => {
  const { t } = useTranslation()
  const [name, setName] = useState(person.name)
  const [personType, setPersonType] = useState(person.personType)
  const [vatPayer, setVatPayer] = useState(person.vatPayer)
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) {
      toast.error(t('validation.nameRequired', { entity: t('persons.name') }))
      return
    }
    setSubmitting(true)
    try {
      await onSubmit(person.id, name.trim(), personType, vatPayer)
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
          <DialogTitle>{t('persons.editPerson')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col flex-1 overflow-hidden min-h-0 gap-4">
          <DialogBody className="flex flex-col gap-4">
            {person.isDefault && <p className="text-sm text-muted-foreground">{t('persons.defaultBadge')}</p>}
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-person-name">{t('persons.name')}</Label>
              <Input id="edit-person-name" type="text" value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="edit-person-type">{t('persons.type')}</Label>
              <Select value={personType} onValueChange={setPersonType}>
                <SelectTrigger id="edit-person-type">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="physical">{t('persons.typePhysical')}</SelectItem>
                  <SelectItem value="legal">{t('persons.typeLegal')}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-center gap-2">
              <Checkbox id="edit-person-vat-payer" checked={vatPayer} onCheckedChange={(v) => setVatPayer(v === true)} />
              <Label htmlFor="edit-person-vat-payer">{t('persons.vatPayer')}</Label>
            </div>
          </DialogBody>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('modals.createPartner.cancel')}
            </Button>
            <Button type="submit" disabled={submitting}>
              {t('persons.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default EditPersonDialog
