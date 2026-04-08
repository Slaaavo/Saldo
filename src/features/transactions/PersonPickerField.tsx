import { useTranslation } from 'react-i18next'
import type { PersonRow } from '../../shared/types'
import { Label } from '../../shared/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../shared/ui/select'

interface Props {
  showPicker: boolean
  resolvedPersonId: number | null
  localPersonId: number | null
  persons: PersonRow[]
  onPersonChange: (id: number) => void
}

const PersonPickerField = ({ showPicker, resolvedPersonId, localPersonId, persons, onPersonChange }: Props) => {
  const { t } = useTranslation()

  if (!showPicker || persons.length === 0) return null

  const value = localPersonId ?? resolvedPersonId

  return (
    <div className="flex flex-col gap-2">
      <Label>{t('persons.selector')}</Label>
      <Select value={value !== null ? String(value) : ''} onValueChange={(v) => onPersonChange(Number(v))}>
        <SelectTrigger>
          <SelectValue placeholder={t('persons.selector')} />
        </SelectTrigger>
        <SelectContent>
          {persons.map((p) => (
            <SelectItem key={p.id} value={String(p.id)}>
              {p.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}

export default PersonPickerField
